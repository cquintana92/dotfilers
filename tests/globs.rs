use crate::test_tools::*;
use dotfilers::{Condition, ConflictStrategy, Directive, DirectiveStep, Executor, LinkDirectoryBehaviour};

#[test]
fn copy_globs_work() {
    run_with_temp_dir(|pb| {
        let a_txt = format!("{}.txt", random_string(10));
        let b_txt = format!("{}.txt", random_string(10));
        let c_file = format!("{}.dat", random_string(10));

        let a_contents = random_string(10);
        let b_contents = random_string(10);
        let c_contents = random_string(10);

        let from_dir_name = random_string(10);
        let to_dir_name = random_string(10);

        let from_dir_path = pb.join(&from_dir_name);
        let to_dir_path = pb.join(&to_dir_name);
        std::fs::create_dir(&from_dir_path).expect("Error creating from dir");
        std::fs::create_dir(&to_dir_path).expect("Error creating to dir");

        write_file(&pb, &format!("{}/{}", from_dir_name, &a_txt), &a_contents);
        write_file(&pb, &format!("{}/{}", from_dir_name, &b_txt), &b_contents);
        write_file(&pb, &format!("{}/{}", from_dir_name, &c_file), &c_contents);

        let executor = Executor::new("", ConflictStrategy::Overwrite);
        executor
            .execute(
                &pb,
                "test",
                &[DirectiveStep {
                    condition: Condition::Always,
                    directive: Directive::Copy {
                        from: format!("{}/*.txt", &from_dir_name),
                        to: to_dir_name,
                    },
                }],
            )
            .expect("Should be able to execute");

        let mut count = 0;
        for f in std::fs::read_dir(&to_dir_path).unwrap() {
            f.unwrap();
            count += 1;
        }

        assert_eq!(count, 2);

        let a_read_contents = std::fs::read_to_string(to_dir_path.join(&a_txt)).unwrap();
        assert_eq!(a_read_contents, a_contents);

        let b_read_contents = std::fs::read_to_string(to_dir_path.join(&b_txt)).unwrap();
        assert_eq!(b_read_contents, b_contents);

        Ok(())
    });
}

#[test]
fn symlink_globs_work() {
    run_with_temp_dir(|pb| {
        let a_txt = format!("{}.txt", random_string(10));
        let b_txt = format!("{}.txt", random_string(10));
        let c_file = format!("{}.dat", random_string(10));

        let a_contents = random_string(10);
        let b_contents = random_string(10);
        let c_contents = random_string(10);

        let from_dir_name = random_string(10);
        let to_dir_name = random_string(10);

        let from_dir_path = pb.join(&from_dir_name);
        let to_dir_path = pb.join(&to_dir_name);
        std::fs::create_dir(&from_dir_path).expect("Error creating from dir");
        std::fs::create_dir(&to_dir_path).expect("Error creating to dir");

        write_file(&pb, &format!("{}/{}", from_dir_name, &a_txt), &a_contents);
        write_file(&pb, &format!("{}/{}", from_dir_name, &b_txt), &b_contents);
        write_file(&pb, &format!("{}/{}", from_dir_name, &c_file), &c_contents);

        let executor = Executor::new("", ConflictStrategy::Overwrite);
        executor
            .execute(
                &pb,
                "test",
                &[DirectiveStep {
                    condition: Condition::Always,
                    directive: Directive::Link {
                        from: format!("{}/*.txt", &from_dir_name),
                        to: to_dir_name,
                        directory_behaviour: LinkDirectoryBehaviour::default(),
                    },
                }],
            )
            .expect("Should be able to execute");

        let mut count = 0;
        for f in std::fs::read_dir(&to_dir_path).unwrap() {
            f.unwrap();
            count += 1;
        }

        assert_eq!(count, 2);

        let a_read_contents = std::fs::read_to_string(to_dir_path.join(&a_txt)).unwrap();
        assert_eq!(a_read_contents, a_contents);

        let b_read_contents = std::fs::read_to_string(to_dir_path.join(&b_txt)).unwrap();
        assert_eq!(b_read_contents, b_contents);

        assert!(to_dir_path.join(&a_txt).is_symlink());
        assert!(to_dir_path.join(&b_txt).is_symlink());

        Ok(())
    });
}
