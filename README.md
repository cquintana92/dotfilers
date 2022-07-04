# Dotfilers

[![Code Quality](https://github.com/cquintana92/dotfilers/actions/workflows/code_quality.yaml/badge.svg)](https://github.com/cquintana92/dotfilers/actions/workflows/code_quality.yaml)
[![Release](https://github.com/cquintana92/dotfilers/actions/workflows/release.yaml/badge.svg)](https://github.com/cquintana92/dotfilers/actions/workflows/release.yaml)

This repository contains the source code for `dotfilers`, a dotfile management utility written in Rust.

## How to get

You can either grab the [latest release](https://github.com/cquintana92/dotfilers/releases/latest) or build it yourself:

```
$ cargo build --release
```

You will find the binary in `target/release/dotfilers`.

Add the binary to your path, and you are ready to go!

## Usage

By default, when invoking `dotfilers` it will look for a `dotfilers.yaml` file in the current directory. However, you can specify a custom file by using `-c/--config PATH`. Keep in mind that the current working directory will be used for calculating relative paths.

When you are testing some configurations, you can pass `-d/--dry-run` in order not to perform any actual operation. If invoked in dry-run mode, dotfilers will print the operations that would be executed, but won't actually perform any operation.

Also, in case you only want to apply some of your `dotfilers.yaml` sections, you can pass the section names as arguments. Let's say you only want to execute your `nvim` and `ssh` sections. In order to do so, you can run `dotfilers nvim ssh`.

## Configuration

### General configuration

You can have a `.dotfilers` section in your `dotfilers.yaml` that will configure behaviours for the `dotfilers` binary.

Here you can find an example with the default values:

```yaml
.dotfilers:
  # Log level for the dotfilers binary
  # Must be one of:
  # - trace
  # - debug
  # - info
  # - warn
  # - error
  log_level: info
  
  # Strategy to use in case of conflict (the file that would be created already exists)
  # Must be one of:
  # - abort (the program will stop)
  # - overwrite (the already existing file/directory will be removed)
  # - rename-old (the already existing file/directory will be renamed to NAME.bak, and in case it also exists, .bak1, .bak2...)
  conflict_strategy: rename-old

  # Shell that will be used for 'run' directives
  shell: /bin/bash -c
```

### Sections configuration

In the root of your `dotfilers.yaml` you can specify the different sections you want to manage.

A section starts with a name, and then a list of operations to be performed. Here you can find an example of all the available operations:

```yaml
# Section for zsh files
zsh:
  # Create a symlink from zsh/.zshrc to ~/.zshrc
  - link_from: zsh/.zshrc
    link_to: ~/.zshrc

  # Only if the os is linux, create a symlink from zsh/.zshrc.linux to ~/.zshrc.local
  - if_os: linux
    link_from: zsh/.zshrc.linux
    link_to: ~/.zshrc.local

  # Only if the os is darwin, create a symlink from zsh/.zshrc.linux to ~/.zshrc.local
  - if_os: darwin
    link_from: zsh/.zshrc.darwin
    link_to: ~/.zshrc.local

# Section for ssh files
ssh:
  # Only if the os is linux:
  # - use the template on ssh/config.tpl
  # - fill it using the vars from ssh/vars_linux
  # - store the result at ~/.ssh/config
  - if_os: linux
    template: ssh/config.tpl
    template_to: ~/.ssh/config
    vars: ssh/vars_linux

  # Only if the os is darwin:
  # - use the template on ssh/config.tpl
  # - fill it using the vars from ssh/vars_darwin
  # - store the result at ~/.ssh/config
  - if_os: darwin
    template: ssh/config.tpl
    template_to: ~/.ssh/config
    vars: ssh/vars_darwin

  # Copy all files that match id_rsa* from the ssh folder into ~/.ssh/
  - copy_from: ssh/id_rsa*
    copy_to: ~/.ssh/

# Section for nvim files
nvim:
  # As this section is too long, run the contents of nvim/deploy.yaml
  - include: nvim/deploy.yaml

# Section for mash files
mash:
  # This will recursively link all files inside the mash directory into ~/mash
  # However, any existing directories will be created rather than symlinked 
  - link_from: mash
    link_to: ~/mash
    link_directory_behaviour: create

# Section for extra things
extra:
  # Run inline code
  - run: echo "abc" > /tmp/abc

  # Run multiple commands
  - run: |
      echo "some_contents" >> /tmp/abc
      echo "Other contents" >> /tmp/abc

  # You can even invoke other programs/scripts
  - run: /usr/bin/python3 -c "print('a' * 127)" > /tmp/lotsofas
```

### Directives

Here you can find a detailed list of all the directives that are supported.

Please notice that if you declare multiple options in the same directive entry (such as `link_from`, `link_to`, `copy_from`, `copy_to`) the resulting operation does not have any guarantee of being consistent, so please avoid doing so.

#### Copy

Copy files or directories from one location to another. This command supports globs in the `copy_from` section.

Sections:

* `copy_from`: Which file / directory to be copied.
  * It supports globs, such as `ssh/id_rsa*` or `directory/*.txt`.
* `copy_to`: Where to copy the files.
  * If the `copy_from` is a single file, please also write the desired destination filename (such as: `copy_to: ~/.ssh/authorized_keys`).
  * If the `copy_from` is a glob, you should use the path to the destination directory (such as: `copy_from: directory/*.txt` and `copy_to: ~/data`).

#### Link

Link files or directories from one location to another. This command supports globs in the `link_from` section.

Sections:

* `link_from`: Which file / directory to be symlinked.
    * It supports globs, such as `ssh/id_rsa*` or `directory/*.txt`.
* `link_to`: Where to symlink the files.
    * If the `link_from` is a single file, please also write the desired destination filename (such as: `link_to: ~/.ssh/authorized_keys`).
    * If the `link_from` is a glob, you should use the path to the destination directory (such as: `link_from: directory/*.txt` and `link_to: ~/data`).
* `link_directory_behaviour`: What to do with directories. If not specified defaults to `link`.
  * If set to `create`, all the directories inside the dir will be created as directories rather than symlinked to the directory. Then, the files inside the original directory will be recursively symlinked.
  * If set to `link`, any directories found will just be symlinked.

#### Template

You can also generate files on the fly by filling templates. `dotfilers` uses [Tera](https://github.com/Keats/tera) as a templating engine, so please refer to the Tera documentation for templates.

In order to define variables, you can create a file with any name you want, and fill the variables with the following format:

```
variable1=value1

# Comments are ignored, but are only supported
# at the beginning of the line
variable2=value2
```

There are some variables that are filled by `dotfilers` itself. For now these variables are:

- `dotfilers_os`: The current OS. May either be `linux` or `darwin`.

#### Run

You can run arbitrary commands with `dotfilers`.

There is no better documentation that some examples:

```yaml
# Run inline code
- run: echo "abc" > /tmp/abc

# Run multiple commands
- run: |
  echo "some_contents" >> /tmp/abc
  echo "Other contents" >> /tmp/abc

# You can even invoke other programs/scripts
- run: /usr/bin/python3 -c "print('a' * 127)" > /tmp/lotsofas
```

Keep in mind that if the exit status code is not 0, the execution will abort.

#### Include

For very long sections it may be handy to delegate the directives into another file. `dotfilers` supports doing so by using the `include` directive.

The included files must have the same structure as the `dotfilers.yaml` file, so that means you will need to wrap your directives into a section.

```yaml
# dotfilers.yaml
nvim:
  - include: nvim/directives.yaml
    
# nvim/directives.yaml
nvim:
  - if_os: linux
    run: echo "This is linux"
```


## License

```
MIT License

Copyright (c) 2022 Carlos Quintana

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```