use crate::test_tools::*;
use dotfilers::{Condition, ConflictStrategy, Directive, DirectiveStep, Executor, LinkDirectoryBehaviour};

#[test]
fn behaviour_link_dir_dest_did_not_exist() {
    run_with_temp_dir(|pb| {
        let original_dir = pb.join("original");
        let dest_dir = pb.join("dest");
        std::fs::create_dir(&original_dir).unwrap();

        let executor = Executor::new("", ConflictStrategy::Overwrite);
        executor
            .execute(
                &pb,
                "test",
                &[DirectiveStep {
                    condition: Condition::Always,
                    directive: Directive::Link {
                        from: original_dir.display().to_string(),
                        to: dest_dir.display().to_string(),
                        directory_behaviour: LinkDirectoryBehaviour::LinkDirectory,
                    },
                }],
            )
            .expect("Should be able to execute");

        assert!(dest_dir.exists());
        assert!(dest_dir.is_symlink());
        assert!(dest_dir.is_dir());

        Ok(())
    });
}

#[test]
fn behaviour_create_dir_dest_did_not_exist() {
    run_with_temp_dir(|pb| {
        let original_dir = pb.join("original");
        let dest_dir = pb.join("dest");
        let dir_root = original_dir.join("dir");
        let dir_in_dir = dir_root.join("dir");
        let dir_in_dir_in_dir = dir_in_dir.join("dir");
        std::fs::create_dir_all(&dir_in_dir_in_dir).unwrap();

        let f0_contents = random_string(10);
        let f1_contents = random_string(10);
        let f2_contents = random_string(10);
        let f3_contents = random_string(10);

        write_file(&original_dir, "file", &f0_contents);
        write_file(&dir_root, "file", &f1_contents);
        write_file(&dir_in_dir, "file", &f2_contents);
        write_file(&dir_in_dir_in_dir, "file", &f3_contents);

        let executor = Executor::new("", ConflictStrategy::Overwrite);
        executor
            .execute(
                &pb,
                "test",
                &[DirectiveStep {
                    condition: Condition::Always,
                    directive: Directive::Link {
                        from: original_dir.display().to_string(),
                        to: dest_dir.display().to_string(),
                        directory_behaviour: LinkDirectoryBehaviour::CreateDirectory,
                    },
                }],
            )
            .expect("Should be able to execute");

        assert!(dest_dir.exists());
        assert!(!dest_dir.is_symlink());

        {
            let dest_dir_contents = dir_contents(&dest_dir);
            assert_eq!(dest_dir_contents.len(), 2);

            let file = dest_dir.join("file");
            assert!(file.exists());
            assert!(file.is_symlink());
            assert!(file.is_file());
            let contents = std::fs::read_to_string(&file).unwrap();
            assert_eq!(contents, f0_contents);

            let dir = dest_dir.join("dir");
            assert!(dir.exists());
            assert!(!dir.is_symlink()); // Should not be a symlink
            assert!(dir.is_dir());
        };

        {
            let dir1 = dest_dir.join("dir");
            let dir_contents = dir_contents(&dir1);
            assert_eq!(dir_contents.len(), 2);

            let file = dir1.join("file");
            assert!(file.exists());
            assert!(file.is_symlink());
            assert!(file.is_file());
            let contents = std::fs::read_to_string(&file).unwrap();
            assert_eq!(contents, f1_contents);

            let dir = dir1.join("dir");
            assert!(dir.exists());
            assert!(!dir.is_symlink());
            assert!(dir.is_dir());
        };

        {
            let dir2 = dest_dir.join("dir").join("dir");
            let dir_contents = dir_contents(&dir2);
            assert_eq!(dir_contents.len(), 2);

            let file = dir2.join("file");
            assert!(file.exists());
            assert!(file.is_symlink());
            assert!(file.is_file());
            let contents = std::fs::read_to_string(&file).unwrap();
            assert_eq!(contents, f2_contents);

            let dir = dir2.join("dir");
            assert!(dir.exists());
            assert!(!dir.is_symlink());
            assert!(dir.is_dir());
        };
        {
            let dir3 = dest_dir.join("dir").join("dir").join("dir");
            let dir_contents = dir_contents(&dir3);
            assert_eq!(dir_contents.len(), 1);

            let file = dir3.join("file");
            assert!(file.exists());
            assert!(file.is_symlink());
            assert!(file.is_file());
            let contents = std::fs::read_to_string(&file).unwrap();
            assert_eq!(contents, f3_contents);
        };

        Ok(())
    });
}

#[test]
fn behaviour_create_dir_dest_existed() {
    run_with_temp_dir(|pb| {
        let original_dir = pb.join("original");
        let dest_dir = pb.join("dest");
        let dir_root = original_dir.join("dir");
        let dir_in_dir = dir_root.join("dir");
        let dir_in_dir_in_dir = dir_in_dir.join("dir");
        std::fs::create_dir_all(&dir_in_dir_in_dir).unwrap();
        std::fs::create_dir(&dest_dir).unwrap(); // Dest dir already exists
        std::fs::create_dir(&dest_dir.join("dir")).unwrap(); // dir inside Dest dir already exists

        let f0_contents = random_string(10);
        let f1_contents = random_string(10);
        let f2_contents = random_string(10);
        let f3_contents = random_string(10);

        write_file(&original_dir, "file", &f0_contents);
        write_file(&dir_root, "file", &f1_contents);
        write_file(&dir_in_dir, "file", &f2_contents);
        write_file(&dir_in_dir_in_dir, "file", &f3_contents);

        // Also add some files in dest_dir so we can check they have not been deleted
        let dest_f0_contents = random_string(10);
        let dest_f1_contents = random_string(10);
        write_file(&dest_dir, "alreadyexisting", &dest_f0_contents);
        write_file(&dest_dir.join("dir"), "alreadyexisting", &dest_f1_contents);

        let executor = Executor::new("", ConflictStrategy::Overwrite);
        executor
            .execute(
                &pb,
                "test",
                &[DirectiveStep {
                    condition: Condition::Always,
                    directive: Directive::Link {
                        from: original_dir.display().to_string(),
                        to: dest_dir.display().to_string(),
                        directory_behaviour: LinkDirectoryBehaviour::CreateDirectory,
                    },
                }],
            )
            .expect("Should be able to execute");

        assert!(dest_dir.exists());
        assert!(!dest_dir.is_symlink());

        {
            let dest_dir_contents = dir_contents(&dest_dir);
            assert_eq!(dest_dir_contents.len(), 3);

            let file = dest_dir.join("file");
            assert!(file.exists());
            assert!(file.is_symlink());
            assert!(file.is_file());
            let contents = std::fs::read_to_string(&file).unwrap();
            assert_eq!(contents, f0_contents);

            let dir = dest_dir.join("dir");
            assert!(dir.exists());
            assert!(!dir.is_symlink()); // Should not be a symlink because is a dir
            assert!(dir.is_dir());

            let already_existing = dest_dir.join("alreadyexisting");
            assert!(already_existing.exists());
            assert!(!already_existing.is_symlink()); // Not a symlink because it already existed
            assert!(already_existing.is_file());
            let contents = std::fs::read_to_string(&already_existing).unwrap();
            assert_eq!(contents, dest_f0_contents);
        };

        {
            let dir1 = dest_dir.join("dir");
            let dir_contents = dir_contents(&dir1);
            assert_eq!(dir_contents.len(), 3);

            let file = dir1.join("file");
            assert!(file.exists());
            assert!(file.is_symlink());
            assert!(file.is_file());
            let contents = std::fs::read_to_string(&file).unwrap();
            assert_eq!(contents, f1_contents);

            let dir = dir1.join("dir");
            assert!(dir.exists());
            assert!(!dir.is_symlink());
            assert!(dir.is_dir());

            let already_existing = dir1.join("alreadyexisting");
            assert!(already_existing.exists());
            assert!(!already_existing.is_symlink()); // Not a symlink because it already existed
            assert!(already_existing.is_file());
            let contents = std::fs::read_to_string(&already_existing).unwrap();
            assert_eq!(contents, dest_f1_contents);
        };

        {
            let dir2 = dest_dir.join("dir").join("dir");
            let dir_contents = dir_contents(&dir2);
            assert_eq!(dir_contents.len(), 2);

            let file = dir2.join("file");
            assert!(file.exists());
            assert!(file.is_symlink());
            assert!(file.is_file());
            let contents = std::fs::read_to_string(&file).unwrap();
            assert_eq!(contents, f2_contents);

            let dir = dir2.join("dir");
            assert!(dir.exists());
            assert!(!dir.is_symlink());
            assert!(dir.is_dir());
        };
        {
            let dir3 = dest_dir.join("dir").join("dir").join("dir");
            let dir_contents = dir_contents(&dir3);
            assert_eq!(dir_contents.len(), 1);

            let file = dir3.join("file");
            assert!(file.exists());
            assert!(file.is_symlink());
            assert!(file.is_file());
            let contents = std::fs::read_to_string(&file).unwrap();
            assert_eq!(contents, f3_contents);
        };

        Ok(())
    });
}
