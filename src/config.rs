use crate::{Error, Result};
use std::collections::HashMap;
use std::path::Path;
use std::str::FromStr;

const SPECIAL_CONFIG_SECTION_NAME: &str = ".dotfilers";
const DEFAULT_LOG_LEVEL: &str = "info";
const DEFAULT_SHELL: &str = "/bin/bash -c";

#[derive(Debug, PartialEq, Eq)]
pub enum ConflictStrategy {
    Abort,
    Overwrite,
    RenameOld,
}

impl FromStr for ConflictStrategy {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "abort" => Ok(Self::Abort),
            "overwrite" => Ok(Self::Overwrite),
            "rename-old" => Ok(Self::RenameOld),
            _ => Err(Error::Config(format!("Unknown ConflictStrategy: {s}"))),
        }
    }
}

#[derive(Debug)]
pub struct ProgramConfig {
    pub shell: String,
    pub log_level: String,
    pub conflict_strategy: ConflictStrategy,
}

impl ProgramConfig {
    pub fn from_yaml(yaml: &str) -> Result<Self> {
        let mut instance = Self::default();
        let parsed: Option<YamlProgramConfig> =
            serde_yaml::from_str(yaml).map_err(|e| Error::Config(format!("Error reading program config: {}", e)))?;
        if let Some(parsed) = parsed {
            if let Some(config) = parsed.config {
                if let Some(log_level) = config.log_level {
                    instance.log_level = log_level;
                }
                if let Some(strat) = config.conflict_strategy {
                    instance.conflict_strategy =
                        ConflictStrategy::from_str(&strat).map_err(|e| Error::Config(format!("Error parsing ConflictStrategy: {}", e)))?;
                }
                if let Some(shell) = config.shell {
                    instance.shell = shell;
                }
            }
        }
        Ok(instance)
    }
}

impl Default for ProgramConfig {
    fn default() -> Self {
        Self {
            shell: DEFAULT_SHELL.to_string(),
            log_level: DEFAULT_LOG_LEVEL.to_string(),
            conflict_strategy: ConflictStrategy::RenameOld,
        }
    }
}

#[derive(Debug, serde::Deserialize)]
struct YamlInnerProgramConfig {
    pub shell: Option<String>,
    pub log_level: Option<String>,
    pub conflict_strategy: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct YamlProgramConfig {
    #[serde(rename = ".dotfilers")]
    config: Option<YamlInnerProgramConfig>,
}

#[derive(Debug, Eq, PartialEq)]
pub enum Os {
    Darwin,
    Linux,
}

impl FromStr for Os {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "darwin" => Ok(Self::Darwin),
            "linux" => Ok(Self::Linux),
            _ => Err(Error::Config(format!("Unknown OS: {s}"))),
        }
    }
}

impl ToString for Os {
    fn to_string(&self) -> String {
        match self {
            Os::Darwin => "darwin",
            Os::Linux => "linux",
        }
        .to_string()
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum Condition {
    Always,
    IfOs(Os),
}

#[derive(Debug, Eq, PartialEq)]
pub struct DirectiveStep {
    pub condition: Condition,
    pub directive: Directive,
}

#[derive(Debug, Eq, PartialEq)]
pub enum LinkDirectoryBehaviour {
    LinkDirectory,
    CreateDirectory,
}

impl Default for LinkDirectoryBehaviour {
    fn default() -> Self {
        LinkDirectoryBehaviour::LinkDirectory
    }
}

impl FromStr for LinkDirectoryBehaviour {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "link" => Ok(Self::LinkDirectory),
            "create" => Ok(Self::CreateDirectory),
            _ => Err(Error::Config(format!("unknown LinkDirectoryBehaviour: {s}"))),
        }
    }
}

impl ToString for LinkDirectoryBehaviour {
    fn to_string(&self) -> String {
        match self {
            LinkDirectoryBehaviour::CreateDirectory => "create",
            LinkDirectoryBehaviour::LinkDirectory => "link",
        }
        .to_string()
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum Directive {
    Link {
        from: String,
        to: String,
        directory_behaviour: LinkDirectoryBehaviour,
    },
    Copy {
        from: String,
        to: String,
    },
    Run(String),
    Include(String),
    Template {
        template: String,
        dest: String,
        vars: Option<String>,
    },
}

#[derive(Debug)]
pub struct StateConfig {
    pub states: HashMap<String, Vec<DirectiveStep>>,
}

#[derive(Debug, serde::Deserialize)]
struct YamlDirectiveStep {
    if_os: Option<String>,
    link_from: Option<String>,
    link_to: Option<String>,
    link_directory_behaviour: Option<String>,
    run: Option<String>,
    include: Option<String>,
    copy_from: Option<String>,
    copy_to: Option<String>,
    template: Option<String>,
    template_to: Option<String>,
    template_vars: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct YamlStateConfig {
    #[serde(rename = ".dotfilers")]
    _ignore: Option<HashMap<String, serde_yaml::Value>>,
    #[serde(flatten)]
    contents: HashMap<String, Vec<YamlDirectiveStep>>,
}

impl StateConfig {
    pub fn from_yaml(contents: &str) -> Result<Self> {
        let parsed: YamlStateConfig = serde_yaml::from_str(contents).map_err(|e| Error::Config(format!("Error parsing yaml: {}", e)))?;
        let mut states = HashMap::new();

        for (k, v) in parsed.contents {
            if k == SPECIAL_CONFIG_SECTION_NAME {
                continue;
            }
            let mut directives = Vec::new();
            for (idx, directive) in v.into_iter().enumerate() {
                let d = Self::parse_directive(directive)
                    .map_err(|e| Error::Config(format!("error parsing directive in section {}, index {}: {}", k, idx, e)))?;
                directives.push(d);
            }
            states.insert(k, directives);
        }

        Ok(Self { states })
    }

    fn parse_directive(d: YamlDirectiveStep) -> Result<DirectiveStep> {
        let condition = Self::extract_condition(&d)?;
        let directive = Self::extract_directive(&d)?;
        Ok(DirectiveStep { condition, directive })
    }

    fn extract_condition(d: &YamlDirectiveStep) -> Result<Condition> {
        match d.if_os {
            None => Ok(Condition::Always),
            Some(ref os) => {
                let parsed_os = Os::from_str(os)?;
                Ok(Condition::IfOs(parsed_os))
            }
        }
    }

    fn extract_directive(d: &YamlDirectiveStep) -> Result<Directive> {
        match (&d.link_from, &d.link_to) {
            (Some(from), Some(to)) => {
                let behaviour = match d.link_directory_behaviour {
                    Some(ref b) => LinkDirectoryBehaviour::from_str(b)
                        .map_err(|e| Error::Config(format!("Error reading LinkDirectoryBehaviour: {}", e)))?,
                    None => LinkDirectoryBehaviour::LinkDirectory,
                };
                return Ok(Directive::Link {
                    from: from.to_string(),
                    to: to.to_string(),
                    directory_behaviour: behaviour,
                });
            }
            (None, None) => {}
            (Some(from), None) => {
                return Err(Error::Config(format!(
                    "Link directive contains only 'from', could not find 'to'. From: {}",
                    from
                )));
            }
            (None, Some(to)) => {
                return Err(Error::Config(format!(
                    "Link directive contains only 'to', could not find 'from'. To: {}",
                    to
                )));
            }
        }
        match (&d.copy_from, &d.copy_to) {
            (Some(from), Some(to)) => {
                return Ok(Directive::Copy {
                    from: from.to_string(),
                    to: to.to_string(),
                });
            }
            (None, None) => {}
            (Some(from), None) => {
                return Err(Error::Config(format!(
                    "Copy directive contains only 'from', could not find 'to'. From: {}",
                    from
                )));
            }
            (None, Some(to)) => {
                return Err(Error::Config(format!(
                    "Copy directive contains only 'to', could not find 'from'. To: {}",
                    to
                )));
            }
        }
        match (&d.template, &d.template_to) {
            (Some(template), Some(dest)) => {
                return Ok(Directive::Template {
                    template: template.to_string(),
                    dest: dest.to_string(),
                    vars: d.template_vars.clone(),
                });
            }
            (None, None) => {}
            (Some(template), None) => {
                return Err(Error::Config(format!(
                    "Template directive contains only 'template', could not find 'template_to'. template: {}",
                    template
                )));
            }
            (None, Some(to)) => {
                return Err(Error::Config(format!(
                    "Template directive contains only 'template_to', could not find 'template'. template_to: {}",
                    to
                )));
            }
        }

        if let Some(ref include) = d.include {
            return Ok(Directive::Include(include.to_string()));
        }

        if let Some(ref run) = d.run {
            return Ok(Directive::Run(run.to_string()));
        }

        Err(Error::Config("Could not find any usable directive".to_string()))
    }
}

#[derive(Debug)]
pub struct Config {
    pub program: ProgramConfig,
    pub state_config: StateConfig,
}

impl Config {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        Self::check_path_valid(path)?;

        let contents = std::fs::read_to_string(path)
            .map_err(|e| Error::Config(format!("Error reading config from path [path={}]: {}", path.display(), e)))?;
        Self::from_yaml(&contents)
    }

    pub fn from_yaml(yaml: &str) -> Result<Self> {
        let program = ProgramConfig::from_yaml(yaml).map_err(|e| Error::Config(format!("Error reading program config: {}", e)))?;
        let state = StateConfig::from_yaml(yaml).map_err(|e| Error::Config(format!("Error reading directives: {}", e)))?;

        Ok(Self {
            program,
            state_config: state,
        })
    }

    fn check_path_valid(path: &Path) -> Result<()> {
        if !path.exists() {
            return Err(Error::Config(format!("Could not find path: {}", path.display())));
        }

        if path.is_dir() {
            return Err(Error::Config(format!("Path is a directory: {}", path.display())));
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn extract_full() {
        let yaml = r#"
.dotfilers:
  shell: someshell
  log_level: debug
  conflict_strategy: overwrite

nvim:
  - if_os: linux
    link_from: ./file_a
    link_to: ~/file_b
        "#;

        let parsed = Config::from_yaml(yaml).expect("Should be able to parse");
        assert_eq!(parsed.program.log_level, "debug");
        assert_eq!(parsed.program.shell, "someshell");
        assert_eq!(parsed.program.conflict_strategy, ConflictStrategy::Overwrite);

        assert_eq!(parsed.state_config.states.len(), 1);

        let nvim = parsed.state_config.states.get("nvim").expect("Should contain an nvim section");
        assert_eq!(nvim.len(), 1);

        assert_eq!(nvim[0].condition, Condition::IfOs(Os::Linux));
        assert_eq!(
            nvim[0].directive,
            Directive::Link {
                from: "./file_a".to_string(),
                to: "~/file_b".to_string(),
                directory_behaviour: LinkDirectoryBehaviour::default(),
            }
        );
    }

    #[test]
    fn extract_directives() {
        let yaml = r#"
nvim:
  - if_os: linux
    link_from: ./file_a
    link_to: ~/file_b
  - if_os: darwin
    run: ./install_macos.sh
  - include: nvim/things.yaml
ssh:
  - if_os: darwin
    run: mkdir -p ~/.ssh
  - if_os: linux
    copy_from: ssh/key
    copy_to: ~/.ssh/key
  - link_from: ssh/config
    link_to: ~/.ssh/config
  - if_os: linux
    template: plugins.tpl
    template_to: plugins
    template_vars: linux_plugins
  - if_os: darwin
    template: plugins.tpl
    template_to: plugins

        "#;
        let parsed = StateConfig::from_yaml(yaml).expect("Should not have failed");

        assert_eq!(parsed.states.len(), 2);

        let nvim = parsed.states.get("nvim").expect("Should contain an nvim section");
        assert_eq!(nvim.len(), 3);

        assert_eq!(nvim[0].condition, Condition::IfOs(Os::Linux));
        assert_eq!(
            nvim[0].directive,
            Directive::Link {
                from: "./file_a".to_string(),
                to: "~/file_b".to_string(),
                directory_behaviour: LinkDirectoryBehaviour::default(),
            }
        );
        assert_eq!(nvim[1].condition, Condition::IfOs(Os::Darwin));
        assert_eq!(nvim[1].directive, Directive::Run("./install_macos.sh".to_string()));
        assert_eq!(nvim[2].condition, Condition::Always);
        assert_eq!(nvim[2].directive, Directive::Include("nvim/things.yaml".to_string()));

        let ssh = parsed.states.get("ssh").expect("Should contain a ssh section");
        assert_eq!(ssh.len(), 5);

        assert_eq!(ssh[0].condition, Condition::IfOs(Os::Darwin));
        assert_eq!(ssh[0].directive, Directive::Run("mkdir -p ~/.ssh".to_string()));
        assert_eq!(ssh[1].condition, Condition::IfOs(Os::Linux));
        assert_eq!(
            ssh[1].directive,
            Directive::Copy {
                from: "ssh/key".to_string(),
                to: "~/.ssh/key".to_string()
            }
        );
        assert_eq!(ssh[2].condition, Condition::Always);
        assert_eq!(
            ssh[2].directive,
            Directive::Link {
                from: "ssh/config".to_string(),
                to: "~/.ssh/config".to_string(),
                directory_behaviour: LinkDirectoryBehaviour::default(),
            }
        );
        assert_eq!(ssh[3].condition, Condition::IfOs(Os::Linux));
        assert_eq!(
            ssh[3].directive,
            Directive::Template {
                template: "plugins.tpl".to_string(),
                vars: Some("linux_plugins".to_string()),
                dest: "plugins".to_string()
            }
        );
        assert_eq!(ssh[4].condition, Condition::IfOs(Os::Darwin));
        assert_eq!(
            ssh[4].directive,
            Directive::Template {
                template: "plugins.tpl".to_string(),
                vars: None,
                dest: "plugins".to_string()
            }
        );
    }

    mod errors {
        use super::*;

        fn expect_error(yaml: &str) {
            StateConfig::from_yaml(yaml).expect_err("Should have failed");
        }

        #[test]
        fn from_without_to() {
            expect_error(
                r#"
nvim:
  - link_from: ./file
            "#,
            )
        }

        #[test]
        fn to_without_from() {
            expect_error(
                r#"
nvim:
  - link_to: ~/file
            "#,
            )
        }

        #[test]
        fn copy_from_without_to() {
            expect_error(
                r#"
nvim:
  - copy_from: ./file
            "#,
            )
        }

        #[test]
        fn copy_to_without_from() {
            expect_error(
                r#"
nvim:
  - copy_to: ~/file
            "#,
            )
        }

        #[test]
        fn unknown_os() {
            expect_error(
                r#"
nvim:
  - if_os: unknown
    run: ./file
            "#,
            )
        }

        #[test]
        fn no_content() {
            expect_error(
                r#"
nvim:
  - {}
            "#,
            )
        }
        #[test]
        fn only_condition() {
            expect_error(
                r#"
nvim:
  - if_os: unknown
            "#,
            )
        }

        #[test]
        fn template_without_template() {
            expect_error(
                r#"
nvim:
  - template_dest: unknown
            "#,
            )
        }

        #[test]
        fn template_without_dest() {
            expect_error(
                r#"
nvim:
  - template: unknown
            "#,
            )
        }
    }
}
