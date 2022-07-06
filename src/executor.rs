use crate::config::{Condition, ConflictStrategy, Directive, DirectiveStep, Os, StateConfig};
use crate::LinkDirectoryBehaviour;
use anyhow::{anyhow, Context, Result};
use fs_extra::dir::CopyOptions;
use std::path::{Path, PathBuf};
use std::process::Command;
use tera::{Context as TeraContext, Tera};

pub trait OsDetector {
    fn get_os(&self) -> Result<Os>;
}

pub struct RealOsDetector;

impl OsDetector for RealOsDetector {
    fn get_os(&self) -> Result<Os> {
        Ok(Os::Linux)
    }
}

pub struct Executor<T>
where
    T: OsDetector,
{
    pub dry_run: bool,
    pub shell: String,
    pub os_detector: T,
    pub conflict_strategy: ConflictStrategy,
}

impl Executor<RealOsDetector> {
    pub fn dry_run(shell: &str, conflict_strategy: ConflictStrategy) -> Self {
        Self::create(true, shell, conflict_strategy)
    }

    pub fn new(shell: &str, conflict_strategy: ConflictStrategy) -> Self {
        Self::create(false, shell, conflict_strategy)
    }

    fn create(dry_run: bool, shell: &str, conflict_strategy: ConflictStrategy) -> Self {
        Self {
            shell: shell.to_string(),
            dry_run,
            os_detector: RealOsDetector,
            conflict_strategy,
        }
    }
}

impl<T> Executor<T>
where
    T: OsDetector,
{
    pub fn execute<P: AsRef<Path>>(&self, root_dir: P, section: &str, directives: &[DirectiveStep]) -> Result<()> {
        let root_dir = root_dir.as_ref();
        if self.dry_run {
            info!("Using root_dir: {}", root_dir.display());
        } else {
            debug!("Using root_dir: {}", root_dir.display());
        }

        for directive in directives {
            debug!("Executing [section={}] [directive={:?}]", section, directive);
            self.execute_directive(root_dir, directive)?;
        }

        info!("Executed section {}", section);
        Ok(())
    }

    fn execute_directive(&self, root_dir: &Path, directive: &DirectiveStep) -> Result<()> {
        match &directive.condition {
            Condition::Always => debug!("Directive has no condition. Executing"),
            Condition::IfOs(os) => {
                let current_os = self.os_detector.get_os().context("Error detecting current OS")?;
                if &current_os == os {
                    debug!("OS match. Executing");
                } else {
                    debug!("OS does not match. Not executing");
                    return Ok(());
                }
            }
        }

        match &directive.directive {
            Directive::Link {
                from,
                to,
                directory_behaviour,
            } => {
                debug!(
                    "Link directive [from={}] [to={}] [behaviour={}]",
                    from,
                    to,
                    directory_behaviour.to_string()
                );
                self.execute_symlink(root_dir, from, to, directory_behaviour)?;
            }
            Directive::Copy { from, to } => {
                debug!("Copy directive [from={}] [to={}]", from, to);
                self.execute_copy(root_dir, from, to)?;
            }
            Directive::Run(cmd) => {
                debug!("Run directive [cmd={}]", cmd);
                self.run(root_dir, cmd)?;
            }
            Directive::Include(path) => {
                debug!("Include directive [path={}]", path);
                self.include(root_dir, path)?;
            }
            Directive::Template { template, dest, vars } => {
                debug!("Template directive [template={}] [dest={}] [vars={:?}]", template, dest, vars);
                self.template(root_dir, template, dest, vars)?;
            }
        }

        Ok(())
    }

    fn execute_symlink(&self, root_dir: &Path, from: &str, to: &str, behaviour: &LinkDirectoryBehaviour) -> Result<()> {
        let paths = self
            .get_paths_to_process(root_dir, from, to)
            .context("Error obtaining paths to process")?;
        let remove_dirs = behaviour.ne(&LinkDirectoryBehaviour::CreateDirectory);
        for (from, to) in paths {
            let (from_path, to_path) = self
                .check_for_conflicts(root_dir, &from, &to, remove_dirs)
                .context("Error in symlink prerequirements")?;
            if from_path.is_dir() {
                match behaviour {
                    LinkDirectoryBehaviour::IgnoreDirectories => {
                        if self.dry_run {
                            info!(
                                "Skipping dir {} as LinkDirectoryBehaviour is set to IgnoreDirectories",
                                from_path.display()
                            );
                        } else {
                            debug!(
                                "Skipping dir {} as LinkDirectoryBehaviour is set to IgnoreDirectories",
                                from_path.display()
                            );
                        }
                    }
                    LinkDirectoryBehaviour::LinkDirectory => {
                        if self.dry_run {
                            info!("Would symlink dir {} -> {}", from_path.display(), to_path.display());
                        } else {
                            symlink::symlink_dir(&from_path, &to_path).context(format!(
                                "Error symlinking dir {} -> {}",
                                from_path.display(),
                                to_path.display()
                            ))?;
                            info!("Symlinked dir {} -> {}", from_path.display(), to_path.display());
                        }
                    }
                    LinkDirectoryBehaviour::CreateDirectory => {
                        if !to_path.exists() {
                            if self.dry_run {
                                info!(
                                    "Would create dir {} as LinkDirectoryBehaviour is set to CreateDirectory",
                                    to_path.display()
                                );
                            } else {
                                debug!(
                                    "Creating dir {} as LinkDirectoryBehaviour is set to CreateDirectory",
                                    to_path.display()
                                );
                                std::fs::create_dir(&to_path).context(format!("Error creating directory {}", to_path.display()))?;
                                info!("Created dir {}", to_path.display());
                            }
                        } else if self.dry_run {
                            info!("To path already exists, no need to do anything {}", to_path.display());
                        } else {
                            debug!("To path already exists, no need to do anything {}", to_path.display());
                        }

                        // Now recurse in files inside from
                        let from_files =
                            std::fs::read_dir(&from_path).context(format!("Error getting dir contents of {}", from_path.display()))?;
                        for entry in from_files {
                            let entry = entry.context(format!("Error getting entry of dir {}", from_path.display()))?;
                            let entry = entry.path();
                            let entry_without_prefix = entry
                                .strip_prefix(&root_dir)
                                .context(format!(
                                    "Error stripping prefix from entry [entry={}] [prefix={}]",
                                    entry.display(),
                                    root_dir.display()
                                ))?
                                .to_path_buf();
                            let from_path = entry_without_prefix.display().to_string();
                            let from_filename = match entry.file_name() {
                                Some(f) => match f.to_str() {
                                    Some(filename) => filename.to_string(),
                                    None => return Err(anyhow!("Cannot convert to str {:?}", f)),
                                },
                                None => return Err(anyhow!("Cannot obtain filename from {}", entry.display())),
                            };
                            let to_path = format!("{}/{}", to, from_filename);
                            self.execute_symlink(root_dir, &from_path, &to_path, behaviour)?;
                        }
                    }
                }
            } else if self.dry_run {
                info!("Would symlink file {} -> {}", from_path.display(), to_path.display());
            } else {
                symlink::symlink_file(&from_path, &to_path).context(format!(
                    "Error symlinking file {} -> {}",
                    from_path.display(),
                    to_path.display()
                ))?;
                info!("Symlinked file {} -> {}", from_path.display(), to_path.display());
            }
        }

        Ok(())
    }

    fn execute_copy(&self, root_dir: &Path, from: &str, to: &str) -> Result<()> {
        let paths = self
            .get_paths_to_process(root_dir, from, to)
            .context("Error obtaining paths to process")?;
        for (from, to) in paths {
            let (from, to) = self
                .check_for_conflicts(root_dir, &from, &to, true)
                .context("Error in copy prerequirements")?;
            if from.is_dir() {
                if self.dry_run {
                    info!("Would copy dir {} -> {}", from.display(), to.display());
                } else {
                    fs_extra::dir::copy(
                        &from,
                        &to,
                        &CopyOptions {
                            overwrite: true,
                            ..CopyOptions::default()
                        },
                    )
                    .context(format!("Error copying dir {} -> {}", from.display(), to.display()))?;
                    info!("Copied dir {} -> {}", from.display(), to.display());
                }
            } else if self.dry_run {
                info!("Would copy file {} -> {}", from.display(), to.display());
            } else {
                debug!("Copying {} -> {}", from.display(), to.display());
                std::fs::copy(&from, &to).context(format!("Error copying file {} -> {}", from.display(), to.display()))?;
                info!("Copied file {} -> {}", from.display(), to.display());
            }
        }

        Ok(())
    }

    fn get_paths_to_process(&self, root_dir: &Path, from: &str, to: &str) -> Result<Vec<(String, String)>> {
        let mut paths = vec![];
        let to_dest = if to.contains('~') {
            PathBuf::from(shellexpand::tilde(to).to_string())
        } else {
            root_dir.join(to)
        };
        if !is_glob(from) {
            paths.push((from.to_string(), to_dest.display().to_string()));
        } else {
            debug!("Detected from is glob: {}", from);
            if !to_dest.exists() {
                // If we have been asked to copy a glob of files to a dir that does not exist, create the dir
                if self.dry_run {
                    info!("Would have created dir {}", to_dest.display());
                } else {
                    debug!("Creating dir {}", to_dest.display());
                    std::fs::create_dir_all(&to_dest).context(format!("Error creating directory {}", to_dest.display()))?;
                }
            } else if !to_dest.is_dir() {
                return Err(anyhow!("Asked to copy into a path that is not a directory"));
            }

            let full_glob = root_dir.join(from).display().to_string();
            debug!("Detected from is glob {} | Will use {}", from, full_glob);
            let glob_iter = glob::glob(&full_glob).context(format!("Error obtaining iterator from glob {}", full_glob))?;
            for entry in glob_iter {
                let entry = entry.context("Error obtaining glob entry")?;
                let entry_without_prefix = entry
                    .strip_prefix(&root_dir)
                    .context("Error stripping prefix from glob")?
                    .to_path_buf();
                let from_path = entry_without_prefix.display().to_string();
                let from_filename = match entry.file_name() {
                    Some(f) => match f.to_str() {
                        Some(filename) => filename.to_string(),
                        None => return Err(anyhow!("Cannot convert to str {:?}", f)),
                    },
                    None => return Err(anyhow!("Cannot obtain filename from {}", entry.display())),
                };
                let to_path = format!("{}/{}", to, from_filename);
                paths.push((from_path, to_path));
            }
        }
        Ok(paths)
    }

    fn check_for_conflicts(&self, root_dir: &Path, from: &str, to: &str, delete_if_dir: bool) -> Result<(PathBuf, PathBuf)> {
        // Check if from file exists
        let from_path = root_dir.join(from);
        let to_path = if to.contains('~') {
            PathBuf::from(shellexpand::tilde(to).to_string())
        } else {
            root_dir.join(to)
        };
        debug!("Checking if 'from' exists: {}", from_path.display());

        if !from_path.exists() {
            return Err(anyhow!("From does not exist: {}", from_path.display()));
        }

        // Check if to already exists
        debug!("Checking if 'to' exists: {}", to_path.display());
        let mut to_already_exists = to_path.exists();
        if !to_already_exists {
            debug!(
                "Detected 'to' does not exist. Checking if is a broken symlink {}",
                to_path.display()
            );
            // Check for broken symlink
            if std::fs::symlink_metadata(&to_path).is_ok() {
                debug!("Detected 'to' is a broken symlink {}", to_path.display());
                to_already_exists = true;
            }
        }

        if to_already_exists {
            debug!("'to' exists: {}", to_path.display());

            // To already exists. Check conflict strategy
            match &self.conflict_strategy {
                ConflictStrategy::Abort => {
                    warn!("ConflictStrategy set to abort. Aborting");
                    return Err(anyhow!(
                        "'to' {} already exists and ConflictStrategy is set to abort",
                        to_path.display()
                    ));
                }
                ConflictStrategy::RenameOld => {
                    debug!("ConflictStrategy set to rename-old. Renaming old");

                    let mut counter = 0;
                    let backup_path = loop {
                        let mut to_path_clone = to_path.display().to_string();
                        let suffix = if counter == 0 {
                            ".bak".to_string()
                        } else {
                            format!(".bak{}", counter)
                        };
                        to_path_clone.push_str(&suffix);

                        let to_path_bak = Path::new(&to_path_clone);
                        debug!("Checking if backup already exists");
                        if !to_path_bak.exists() {
                            break to_path_clone;
                        }
                        counter += 1;
                    };

                    let backup_path = Path::new(&backup_path);
                    if self.dry_run {
                        info!("Would move [src={}] [dst={}]", to_path.display(), backup_path.display());
                    } else {
                        warn!("Moving [src={}] -> [dst={}]", to_path.display(), backup_path.display());
                        std::fs::rename(&to_path, backup_path).context(format!(
                            "Error renaming [src={}] -> [dst={}]",
                            to_path.display(),
                            backup_path.display()
                        ))?;
                    }

                    return Ok((from_path, to_path));
                }
                ConflictStrategy::Overwrite => {
                    if to_path.is_symlink() {
                        if to_path.is_file() {
                            if self.dry_run {
                                info!("Would remove file symlink {}", to_path.display());
                            } else {
                                warn!("Removing file symlink {}", to_path.display());
                                symlink::remove_symlink_file(&to_path)
                                    .context(format!("Error removing file symlink {}", to_path.display()))?;
                            }
                        } else if to_path.is_dir() {
                            if delete_if_dir {
                                if self.dry_run {
                                    info!("Would remove dir symlink {}", to_path.display());
                                } else {
                                    warn!("Removing dir symlink {}", to_path.display());
                                    symlink::remove_symlink_dir(&to_path)
                                        .context(format!("Error removing dir symlink {}", to_path.display()))?;
                                }
                            } else if self.dry_run {
                                info!(
                                    "Would not remove dir symlink as is specified in configuration {}",
                                    to_path.display()
                                );
                            } else {
                                debug!("Not removing dir symlink as is specified in configuration {}", to_path.display());
                            }
                        } else {
                            // Probably a broken symlink if is neither a file nor a dir
                            if self.dry_run {
                                info!("Would remove broken symlink {}", to_path.display());
                            } else {
                                warn!("Removing broken symlink {}", to_path.display());
                                symlink::remove_symlink_file(&to_path).context("Error removing broken symlink")?;
                            }
                        }
                    } else if to_path.is_file() {
                        if self.dry_run {
                            info!("Would remove file {}", to_path.display());
                        } else {
                            warn!("Removing file {}", to_path.display());
                            std::fs::remove_file(&to_path).context(format!("Error removing file {}", to_path.display()))?;
                        }
                    } else if to_path.is_dir() {
                        if delete_if_dir {
                            if self.dry_run {
                                info!("Would remove dir {}", to_path.display());
                            } else {
                                warn!("Removing dir {}", to_path.display());
                                std::fs::remove_dir_all(&to_path).context(format!("Error removing dir {}", to_path.display()))?;
                            }
                        } else if self.dry_run {
                            info!("Would not remove dir as is specified in configuration {}", to_path.display());
                        } else {
                            debug!("Not removing dir as is specified in configuration {}", to_path.display());
                        }
                    } else if self.dry_run {
                        info!("Would remove dir {}", to_path.display());
                    } else {
                        warn!("Removing dir {}", to_path.display());
                        std::fs::remove_dir_all(&to_path).context(format!("Error removing dir {}", to_path.display()))?;
                    }
                }
            }
        } else {
            debug!("To {} does not exist", to_path.display());
            // Check if parent dir structure exists
            if let Some(parent) = to_path.parent() {
                if !parent.exists() {
                    if self.dry_run {
                        info!("As parent dir does not exist, would have created {}", parent.display());
                    } else {
                        debug!("Creating parent dir structure {}", parent.display());
                        std::fs::create_dir_all(&parent).context(format!("Error creating parent dir structure {}", parent.display()))?;
                    }
                }
            }
        }

        Ok((from_path, to_path))
    }

    fn run(&self, root_dir: &Path, cmd: &str) -> Result<()> {
        let current_dir = Path::new(root_dir);
        let shell_args = self.shell.split(' ').collect::<Vec<&str>>();
        if shell_args.is_empty() {
            return Err(anyhow!("Cannot run commands with an empty shell definition"));
        }

        let mut command = Command::new(shell_args[0]);
        for arg in shell_args.iter().skip(1) {
            command.arg(arg);
        }
        command.arg(cmd).current_dir(current_dir);

        if self.dry_run {
            info!(
                "Would run [current_dir={}]: {:?} {:?}",
                root_dir.display(),
                command.get_program(),
                command.get_args()
            );
        } else {
            debug!("Command to be executed: {:?} {:?}", command.get_program(), command.get_args());
            let mut exec = command.spawn().context("Error invoking subcommand")?;
            let exit_status = exec.wait().context("Error waiting for subcommand to finish")?;
            if let Some(code) = exit_status.code() {
                debug!("Command exit status: {}", code);
                if code != 0 {
                    return Err(anyhow!(
                        "Command exit status was not 0. Exit status: {} | Command: {:?} {:?}",
                        exit_status,
                        command.get_program(),
                        command.get_args()
                    ));
                }
            }

            info!("Executed command {}", cmd);
        }

        Ok(())
    }

    fn include(&self, root_dir: &Path, path: &str) -> Result<()> {
        let yaml_path = root_dir.join(path);
        if !yaml_path.exists() {
            return Err(anyhow!("Could not file yaml to include in path {}", yaml_path.display()));
        }

        let contents = std::fs::read_to_string(&yaml_path).context(format!("Error loading included file {}", yaml_path.display()))?;
        let config = StateConfig::from_yaml(&contents).context(format!("Error parsing included file {}", yaml_path.display()))?;

        let included_root_dir = match yaml_path.parent() {
            Some(p) => p,
            None => root_dir,
        };
        debug!("Using root_dir: {}", included_root_dir.display());

        for (section, directives) in config.states {
            self.execute(included_root_dir, &section, &directives)
                .context(format!("Error executing directives from file {}", yaml_path.display()))?;
        }

        info!("Finished include section {}", path);

        Ok(())
    }

    fn template(&self, root_dir: &Path, template: &str, dest: &str, vars: &Option<String>) -> Result<()> {
        let (template, dest) = self
            .check_for_conflicts(root_dir, template, dest, true)
            .context("Error preparing files for templating")?;

        let template_contents =
            std::fs::read_to_string(&template).context(format!("Error reading template contents: {}", template.display()))?;
        let mut tera = Tera::default();
        let mut context = TeraContext::new();
        // Add default variables
        let os = self.os_detector.get_os().context("Error detecting current os")?;
        context.insert("dotfilers_os", &os.to_string());
        load_vars_into_context(root_dir, vars, &mut context).context("Error loading template vars")?;

        let rendered = tera.render_str(&template_contents, &context).context("Error rendering template")?;

        if self.dry_run {
            info!("Would have written into {} the following template: {}", dest.display(), rendered);
        } else {
            debug!("Writing template into {}", dest.display());
            std::fs::write(&dest, rendered).context(format!("Error writing templated contents into {}", dest.display()))?;
            info!("Rendered file {}", dest.display());
        }

        Ok(())
    }
}

fn is_glob(path: &str) -> bool {
    path.contains('*')
}

fn load_vars_into_context(root_dir: &Path, vars: &Option<String>, context: &mut TeraContext) -> Result<()> {
    let vars = match vars {
        Some(v) => v,
        None => return Ok(()),
    };

    let vars_path = root_dir.join(vars);

    if !vars_path.exists() {
        return Err(anyhow!("Could not find vars file {}", vars_path.display()));
    }

    if !vars_path.is_file() {
        return Err(anyhow!("Vars file is not a file {}", vars_path.display()));
    }

    let vars_contents = std::fs::read_to_string(&vars_path).context(format!("Error reading vars file {}", vars_path.display()))?;
    load_vars_into_context_from_str(&vars_contents, context);

    Ok(())
}

fn load_vars_into_context_from_str(contents: &str, context: &mut TeraContext) {
    for line in contents.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let splits = line.split_once('=');
        if let Some((name, value)) = splits {
            context.insert(name.to_string(), &value.to_string());
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn load_vars_into_context() {
        let mut context = TeraContext::new();
        load_vars_into_context_from_str(
            r#"
name=test
number=123
withequals=a=b=c
# contents=abc
        "#,
            &mut context,
        );

        assert_eq!(context.get("name"), Some(&tera::Value::String("test".to_string())));
        assert_eq!(context.get("number"), Some(&tera::Value::String("123".to_string())));
        assert_eq!(context.get("withequals"), Some(&tera::Value::String("a=b=c".to_string())));

        // Assert there are only 3 sections, as the comment is ignored
        let as_json = context.into_json();
        let v = as_json.as_object().unwrap();
        assert_eq!(v.len(), 3);
    }
}
