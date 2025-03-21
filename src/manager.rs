use crate::{
    Config, Engine, Kind,
    error::{AppError, AppErrorKind},
};
use serde::Deserialize;
use std::{
    fs,
    path::{self, Path, PathBuf},
};

#[derive(Deserialize, Debug, PartialEq)]
pub struct Manager {
    configs: Vec<Config>,
    dryrun: bool,
}

impl Manager {
    pub fn new() -> Manager {
        Manager {
            configs: vec![],
            dryrun: false,
        }
    }

    pub fn validate(&mut self, engine: Engine) -> crate::Result<()> {
        // dryrun
        self.dryrun = engine.dryrun;

        // config
        if let Some(mut path) = engine.config {
            // check relative or absolute path
            path = if path.is_relative() {
                path::absolute(path)?
            } else {
                path
            };

            // config file exists or not
            if !path.exists() {
                return Err(AppError::new(
                    AppErrorKind::Usage,
                    "config file doesn't exists",
                ));
            }

            // config file is a json file or not?
            let extn = path
                .extension()
                .unwrap_or_default()
                .to_str()
                .unwrap_or_default()
                .to_lowercase();
            if extn != "json" {
                return Err(AppError::new(
                    AppErrorKind::Usage,
                    "config file is not a JSON file, please provide a JSON file",
                ));
            }

            // parse config file
            self.parse(path)?;
            Ok(())
        } else {
            let destination = engine.destination.ok_or(AppError::new(
                AppErrorKind::Usage,
                "Please provide destination",
            ))?;
            let kind = engine
                .kind
                .ok_or(AppError::new(AppErrorKind::Usage, "Please provide kind"))?;

            let patterns = engine.patterns.ok_or(AppError::new(
                AppErrorKind::Usage,
                "Please provide patterns",
            ))?;

            // validate destination path exists or not
            if !destination.exists() {
                return Err(AppError::new(
                    AppErrorKind::Usage,
                    "destination doesn't exists",
                ));
            }

            // make sure destination path is a folder, not file or symlink
            if !destination.is_dir() {
                return Err(AppError::new(
                    AppErrorKind::Usage,
                    "destination is not a directory, please provide directory path as destination!",
                ));
            }

            // format user input
            self.format(destination, kind, patterns, engine.exclude)?;
            Ok(())
        }
    }

    pub fn execute(&self) -> crate::Result<()> {
        // loop over each config
        for config in &self.configs {
            helper::remove(
                &config.destination,
                &config.kind,
                &config.patterns,
                &config.exclude.clone().unwrap_or_default(),
                self.dryrun,
            );

            // let mut item = helper::Remove {
            //     destination: config.destination.clone(),
            //     kind: config.kind.clone(),
            //     patterns: config.patterns.clone(),
            //     dryrun: self.dryrun,
            // };
            // helper::remove_as_mut(&mut item);
        }
        Ok(())
    }

    fn add(&mut self, config: Config) {
        self.configs.push(config);
    }

    fn parse<T: AsRef<Path>>(&mut self, path: T) -> crate::Result<()> {
        let json_data = fs::read_to_string(path)?;
        self.configs = serde_json::from_str(&json_data)?;
        Ok(())
    }

    fn format<T: Into<PathBuf>>(
        &mut self,
        destination: T,
        kind: Kind,
        patterns: Vec<String>,
        exclude: Option<Vec<String>>,
    ) -> crate::Result<()> {
        self.add(Config::new(destination.into(), kind, patterns, exclude));
        Ok(())
    }
}

impl Default for Manager {
    fn default() -> Self {
        Self::new()
    }
}

mod helper {
    use super::*;

    #[allow(dead_code)]
    pub struct Remove {
        pub destination: PathBuf,
        pub kind: Kind,
        pub patterns: Vec<String>,
        pub exclude: Vec<String>,
        pub dryrun: bool,
    }

    impl AsMut<Remove> for Remove {
        fn as_mut(&mut self) -> &mut Remove {
            self
        }
    }

    #[allow(dead_code)]
    pub fn remove_as_mut<T: AsMut<Remove>>(item: &mut T) {
        let item = item.as_mut();

        if item.destination.exists() {
            // get child item of kind
            let children = self::childern(&item.destination, &item.exclude);

            // iterate over each child
            for child in &children {
                // if match, then remove
                match self::pattern_check(child, &item.patterns, &item.kind) {
                    Some(_) => {
                        // remove child
                        println!("Removing {:?}...", child);
                        if !&item.dryrun {
                            match self::remove_item(child) {
                                Ok(_) => println!("Removed {:?}...", child),
                                Err(e) => eprintln!("Error: {}", e),
                            }
                        }
                    }
                    None => {
                        if child.is_dir() {
                            item.destination = child.to_path_buf();
                            self::remove_as_mut(item);
                        }
                    }
                }
            }
        }
    }

    // TODO: think remove need to return Result<...>?
    pub fn remove<P: AsRef<Path>>(
        destination: P,
        kind: &Kind,
        patterns: &[String],
        exclude: &[String],
        dryrun: bool,
    ) {
        // pub fn remove(destination: &Path, kind: &Kind, patterns: &[String], dryrun: bool) {
        let destination = destination.as_ref();
        if destination.exists() {
            // get child item of kind
            let children = self::childern(destination, exclude);

            // iterate over each child
            for child in &children {
                // if match, then remove
                match self::pattern_check(child, patterns, kind) {
                    Some(_) => {
                        // remove child
                        println!("\u{1b}[91mRemoving\u{1b}[0m {:?}...", child);
                        if !dryrun {
                            match self::remove_item(child) {
                                Ok(_) => println!("\u{1b}[31mRemoved\u{1b}[0m {:?}...", child),
                                Err(e) => eprintln!("Error: {}", e),
                            }
                        }
                    }
                    None => {
                        if child.is_dir() {
                            self::remove(child, kind, patterns, exclude, dryrun);
                        }
                    }
                }
            }
        }
    }

    // TODO: return Result<Vec<PathBuf>, AppError>
    pub fn childern<P: AsRef<Path>>(parent: P, exclude: &[String]) -> Vec<PathBuf> {
        let mut children = Vec::new();

        match fs::read_dir(parent) {
            Ok(entries) => {
                for entry in entries {
                    match entry {
                        Ok(entry) => {
                            // don't add path that exists in exclude list
                            let path = entry.path();
                            let name = path
                                .file_name()
                                .unwrap_or_default()
                                .to_str()
                                .unwrap_or_default();
                            match self::find(name, exclude) {
                                Some(_) => {
                                    // println!("index: {:?}", index);
                                    // println!("path: {:?}", name);
                                    // println!("exclude: {:?}", exclude);

                                    println!("\u{1b}[33mExclude\u{1b}[0m {:?}...", path);
                                }
                                None => children.push(path),
                            }

                            // check child is matching with patterns or not
                            // if *kind == Kind::Folder && entry.file_type().unwrap().is_dir() {
                            //     children.push(entry.path());
                            // } else if *kind == Kind::File && entry.file_type().unwrap().is_file() {
                            //     children.push(entry.path());
                            // }
                        }
                        Err(e) => {
                            eprintln!("Error reading directory entry: {}", e);
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("Error reading directory: {}", e);
            }
        }

        children
    }

    fn find<T: AsRef<str>>(item: T, list: &[String]) -> Option<usize> {
        let item = item.as_ref();
        list.iter()
            .position(|n| n.to_lowercase() == item.to_lowercase())
    }

    pub fn pattern_check<P: AsRef<Path>>(
        path: P,
        patterns: &[String],
        kind: &Kind,
    ) -> Option<usize> {
        let path = path.as_ref();
        // check for folder
        if *kind == Kind::Folder && path.is_dir() {
            let name = path
                .file_name()
                .unwrap_or_default()
                .to_str()
                .unwrap_or_default();
            self::find(name, patterns)
            // patterns
            //     .iter()
            //     .position(|n| n.to_lowercase() == name.to_lowercase())
        } else if *kind == Kind::File && path.is_file() {
            let extn = path
                .extension()
                .unwrap_or_default()
                .to_str()
                .unwrap_or_default();
            self::find(extn, patterns)
            // patterns
            //     .iter()
            //     .position(|n| n.to_lowercase() == extn.to_lowercase())
        } else {
            None
        }
    }

    pub fn remove_item<P: AsRef<Path>>(path: P) -> std::io::Result<()> {
        if path.as_ref().is_file() {
            fs::remove_file(path)
        } else {
            fs::remove_dir_all(path)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_manager() {
        let manager = Manager::new();
        assert_eq!(
            manager,
            Manager {
                configs: vec![],
                dryrun: false
            }
        );
    }

    #[test]
    fn add_config() {
        let mut manager = Manager::new();
        manager.add(Config::new(
            "/Users/abhinath/productive/pool/Project",
            Kind::Folder,
            vec!["build", "debug", "release"],
            None,
        ));
        assert_eq!(
            manager,
            Manager {
                configs: vec![Config {
                    destination: PathBuf::from("/Users/abhinath/productive/pool/Project"),
                    kind: Kind::Folder,
                    patterns: vec![
                        String::from("build"),
                        String::from("debug"),
                        String::from("release"),
                    ],
                    exclude: None,
                }],
                dryrun: false
            }
        );
    }

    #[test]
    fn check_format() {
        let mut manager = Manager::new();
        manager
            .format(
                "/Users/abhinath/productive/pool/Project",
                Kind::Folder,
                vec![
                    String::from("build"),
                    String::from("debug"),
                    String::from("release"),
                ],
                None,
            )
            .unwrap();

        assert_eq!(
            manager,
            Manager {
                configs: vec![Config {
                    destination: PathBuf::from("/Users/abhinath/productive/pool/Project"),
                    kind: Kind::Folder,
                    patterns: vec![
                        String::from("build"),
                        String::from("debug"),
                        String::from("release"),
                    ],
                    exclude: None,
                }],
                dryrun: false
            }
        );
    }

    #[test]
    // #[should_panic]
    fn check_remove_as_mut() {
        let mut item = helper::Remove {
            destination: PathBuf::from("/Users/abhinath/productive/pool"),
            kind: Kind::Folder,
            patterns: vec![
                String::from("packages"),
                String::from("bin"),
                String::from("obj"),
                String::from("Debug"),
                String::from("Release"),
            ],
            exclude: vec![],
            dryrun: true,
        };
        helper::remove_as_mut(&mut item);
    }
}
