use crate::test_tools::*;
use dotfilers::{Condition, ConflictStrategy, Directive, DirectiveStep, Executor};

#[test]
fn template_works() {
    run_with_temp_dir(|pb| {
        let template_contents = r#"
Some text
Created with os {{ dotfilers_os }}
Here is a variable: {{ name }}
"#;
        let variable_contents = r#"
name=test
"#;
        let expected = r#"
Some text
Created with os linux
Here is a variable: test
"#;

        let template_filename = random_string(10);
        let variable_filename = random_string(10);
        write_file(&pb, &template_filename, template_contents);
        write_file(&pb, &variable_filename, variable_contents);

        let dest_filename = random_string(10);
        let executor = Executor::new("", ConflictStrategy::Overwrite);
        executor
            .execute(
                &pb,
                "test",
                &[DirectiveStep {
                    condition: Condition::Always,
                    directive: Directive::Template {
                        template: template_filename,
                        dest: dest_filename.clone(),
                        vars: Some(variable_filename),
                    },
                }],
            )
            .expect("Should be able to execute");

        let dest = pb.join(dest_filename);
        assert!(dest.exists());

        let dest_contents = std::fs::read_to_string(dest).expect("Should be able to read dest contents");
        assert_eq!(dest_contents, expected);

        Ok(())
    });
}
