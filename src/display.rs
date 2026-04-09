use crate::config::{Metadata, Step};
use console::style;

pub fn print_banner(meta: &Metadata) {
    println!();
    println!("{}", style(&meta.name).bold().cyan());
    if let Some(desc) = &meta.description {
        println!("{}", style(desc).dim());
    }
    println!();
}

pub fn print_step_header(step: &Step) {
    println!("{} {}", style("▶").cyan(), style(&step.name).bold());
}

pub fn print_success(step: &Step) {
    println!("{} {}", style("✓").green().bold(), style(&step.name).green());
}

pub fn print_failure(step: &Step) {
    println!("{} {}", style("✗").red().bold(), style(&step.name).red());
}

pub fn print_skip(step: &Step) {
    println!("{} {} (skipped)", style("–").yellow(), style(&step.name).yellow());
}

pub fn print_dry_run_header(step: &Step) {
    println!("{} {}", style("[dry]").dim(), style(&step.name).bold());
}

pub fn print_step_list(steps: &[Step]) {
    let mut grouped: Vec<(Option<&str>, Vec<&Step>)> = vec![];

    for step in steps {
        let group = step.group.as_deref();
        if let Some(entry) = grouped.iter_mut().find(|(g, _)| *g == group) {
            entry.1.push(step);
        } else {
            grouped.push((group, vec![step]));
        }
    }

    for (group, group_steps) in &grouped {
        if let Some(g) = group {
            println!("\n{}", style(g).bold().underlined());
        } else {
            println!();
        }
        for step in group_steps {
            let confirm_marker = if step.confirm {
                style(" [confirm]").yellow().to_string()
            } else {
                String::new()
            };
            let dep_marker = if !step.depends_on.is_empty() {
                style(format!(" [needs: {}]", step.depends_on.join(", ")))
                    .dim()
                    .to_string()
            } else {
                String::new()
            };
            println!(
                "  {} {}{}{}",
                style("•").dim(),
                style(&step.name).bold(),
                confirm_marker,
                dep_marker
            );
            println!("    {}", style(&step.description).dim());
        }
    }
    println!();
}
