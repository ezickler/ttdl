#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate serde_derive;

mod conf;
mod fmt;
mod tml;

use std::env;
use std::path::Path;
use std::process::exit;
use std::str::FromStr;

use todo_lib::*;

type FnUpdateData = fn(tasks: &mut Vec<todo_txt::task::Extended>, ids: Option<&todo::IDVec>) -> todo::ChangedVec;

fn calculate_updated(v: &todo::ChangedSlice) -> u32 {
    let mut cnt = 0u32;
    for b in v.iter() {
        if *b {
            cnt += 1;
        }
    }

    cnt
}

fn process_tasks(tasks: &mut todo::TaskVec, c: &conf::Conf, action: &str, f: FnUpdateData) -> bool {
    let todos = tfilter::filter(&tasks, &c.flt);

    if c.dry {
        let mut clones = todo::clone_tasks(&tasks, &todos);
        let updated = f(&mut clones, None);
        let updated_cnt = calculate_updated(&updated);

        if updated_cnt == 0 {
            println!("No todo was {}", action);
        } else {
            println!("Todos to be {}:", action);
            fmt::print_header(&c.fmt);
            fmt::print_todos(&tasks, &todos, &updated, &c.fmt, false);
            println!("\nReplace with:");
            fmt::print_todos(&clones, &todos, &updated, &c.fmt, true);
            fmt::print_footer(&tasks, &todos, &updated, &c.fmt);
        }
        false
    } else {
        let updated = f(tasks, Some(&todos));
        let updated_cnt = calculate_updated(&updated);

        if updated_cnt == 0 {
            println!("No todo was {}", action);
            false
        } else {
            println!("Changed todos:");
            fmt::print_header(&c.fmt);
            fmt::print_todos(&tasks, &todos, &updated, &c.fmt, false);
            fmt::print_footer(&tasks, &todos, &updated, &c.fmt);
            true
        }
    }
}

fn task_add(tasks: &mut todo::TaskVec, conf: &conf::Conf) {
    let subj = match &conf.todo.subject {
        None => {
            eprintln!("Subject is empty");
            return;
        }
        Some(s) => s.clone(),
    };
    if conf.dry {
        match todo_txt::task::Extended::from_str(&subj) {
            Ok(t) => {
                println!("To be added: ");
                fmt::print_header(&conf.fmt);
                fmt::print_todos(&[t], &[tasks.len()], &[true], &conf.fmt, true);
            }
            Err(e) => {
                eprintln!("Invalid format: {:?}", e);
                exit(1);
            }
        }

        return;
    }

    let id = todo::add(tasks, &conf.todo);
    if id == todo::INVALID_ID {
        println!("Failed to add: parse error '{}'", subj);
        std::process::exit(1);
    }

    println!("Added todo: {}", id);
    fmt::print_header(&conf.fmt);
    fmt::print_todos(&tasks, &[id], &[true], &conf.fmt, false);
    if let Err(e) = todo::save(&tasks, Path::new(&conf.todo_file)) {
        println!("Failed to save to '{:?}': {}", &conf.todo_file, e);
        std::process::exit(1);
    }
}

fn task_list(tasks: &todo::TaskSlice, conf: &conf::Conf) {
    let mut todos = tfilter::filter(tasks, &conf.flt);
    tsort::sort(&mut todos, tasks, &conf.sort);
    fmt::print_header(&conf.fmt);
    fmt::print_todos(tasks, &todos, &[], &conf.fmt, false);
    fmt::print_footer(tasks, &todos, &[], &conf.fmt);
}

fn task_done(tasks: &mut todo::TaskVec, conf: &conf::Conf) {
    if process_tasks(tasks, conf, "completed", todo::done) {
        if let Err(e) = todo::save(tasks, Path::new(&conf.todo_file)) {
            println!("Failed to save to '{:?}': {}", &conf.todo_file, e);
            std::process::exit(1);
        }
    }
}

fn task_undone(tasks: &mut todo::TaskVec, conf: &conf::Conf) {
    let mut flt_conf = conf.clone();
    if flt_conf.flt.all == tfilter::TodoStatus::Active {
        flt_conf.flt.all = tfilter::TodoStatus::Done;
    }

    if process_tasks(tasks, &flt_conf, "uncompleted", todo::undone) {
        if let Err(e) = todo::save(tasks, Path::new(&flt_conf.todo_file)) {
            println!("Failed to save to '{:?}': {}", &flt_conf.todo_file, e);
            std::process::exit(1);
        }
    }
}

fn task_remove(tasks: &mut todo::TaskVec, conf: &conf::Conf) {
    let mut flt_conf = conf.clone();
    if flt_conf.flt.all == tfilter::TodoStatus::Active {
        flt_conf.flt.all = tfilter::TodoStatus::All;
    }
    let todos = tfilter::filter(tasks, &flt_conf.flt);
    if todos.is_empty() {
        println!("No todo deleted")
    } else {
        if flt_conf.dry {
            println!("Todos to be removed:")
        } else {
            println!("Removed todos:")
        }
        fmt::print_header(&conf.fmt);
        fmt::print_todos(&tasks, &todos, &[], &conf.fmt, false);
        fmt::print_footer(tasks, &todos, &[], &conf.fmt);
        if !flt_conf.dry {
            let removed = todo::remove(tasks, Some(&todos));
            if calculate_updated(&removed) != 0 {
                if let Err(e) = todo::save(tasks, Path::new(&flt_conf.todo_file)) {
                    println!("Failed to save to '{:?}': {}", &flt_conf.todo_file, e);
                    std::process::exit(1);
                }
            }
        }
    }
}

fn task_clean(tasks: &mut todo::TaskVec, conf: &conf::Conf) {
    let flt_conf = tfilter::Conf {
        all: tfilter::TodoStatus::Done,
        ..conf.flt.clone()
    };
    let todos = tfilter::filter(tasks, &flt_conf);
    if todos.is_empty() {
        println!("No todo archived")
    } else {
        if conf.dry {
            println!("Todos to be archived:")
        } else {
            println!("Archived todos:")
        }
        fmt::print_header(&conf.fmt);
        fmt::print_todos(&tasks, &todos, &[], &conf.fmt, false);
        fmt::print_footer(tasks, &todos, &[], &conf.fmt);
        if !conf.dry {
            let cloned = todo::clone_tasks(tasks, &todos);
            if !conf.wipe {
                if let Err(e) = todo::archive(&cloned, &conf.done_file) {
                    eprintln!("{:?}", e);
                    exit(1);
                }
            }
            let removed = todo::remove(tasks, Some(&todos));
            if calculate_updated(&removed) != 0 {
                if let Err(e) = todo::save(tasks, Path::new(&conf.todo_file)) {
                    println!("Failed to save to '{:?}': {}", &conf.todo_file, e);
                    std::process::exit(1);
                }
            }
        }
    }
}

fn task_edit(tasks: &mut todo::TaskVec, conf: &conf::Conf) {
    let todos = tfilter::filter(tasks, &conf.flt);
    let action = "changed";
    if todos.is_empty() {
        println!("No todo changed")
    } else if conf.dry {
        let mut clones = todo::clone_tasks(tasks, &todos);
        let updated = todo::edit(&mut clones, None, &conf.todo);
        let updated_cnt = calculate_updated(&updated);

        if updated_cnt == 0 {
            println!("No todo was {}", action);
        } else {
            println!("Todos to be {}:", action);
            fmt::print_header(&conf.fmt);
            fmt::print_todos(&tasks, &todos, &updated, &conf.fmt, false);
            println!("\nNew todos:");
            fmt::print_todos(&clones, &todos, &updated, &conf.fmt, true);
            fmt::print_footer(&tasks, &todos, &updated, &conf.fmt);
        }
    } else {
        let updated = todo::edit(tasks, Some(&todos), &conf.todo);
        let updated_cnt = calculate_updated(&updated);

        if updated_cnt == 0 {
            println!("No todo was {}", action);
        } else {
            println!("Changed todos:");
            fmt::print_header(&conf.fmt);
            fmt::print_todos(&tasks, &todos, &updated, &conf.fmt, false);
            fmt::print_footer(&tasks, &todos, &updated, &conf.fmt);
            if let Err(e) = todo::save(tasks, Path::new(&conf.todo_file)) {
                println!("Failed to save to '{:?}': {}", &conf.todo_file, e);
                std::process::exit(1);
            }
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();

    let mut conf = match conf::parse_args(&args) {
        Ok(c) => c,
        Err(e) => {
            println!("{}", e);
            exit(1);
        }
    };

    // println!("{:#?}", conf);

    let mut tasks: todo::TaskVec = if conf.use_done {
        match todo::load(Path::new(&conf.done_file)) {
            Ok(tlist) => tlist,
            Err(e) => {
                eprintln!("Failed to load todo list: {:?}", e);
                exit(1);
            }
        }
    } else {
        match todo::load(Path::new(&conf.todo_file)) {
            Ok(tlist) => tlist,
            Err(e) => {
                eprintln!("Failed to load done list: {:?}", e);
                exit(1);
            }
        }
    };
    conf.fmt.max = tasks.len();

    if conf.mode != conf::RunMode::List && conf.use_done {
        eprintln!("Invalid command: when using done.txt the only available command is `list`");
        exit(1);
    }

    match conf.mode {
        conf::RunMode::Add => task_add(&mut tasks, &conf),
        conf::RunMode::List => task_list(&tasks, &conf),
        conf::RunMode::Done => task_done(&mut tasks, &conf),
        conf::RunMode::Undone => task_undone(&mut tasks, &conf),
        conf::RunMode::Remove => task_remove(&mut tasks, &conf),
        conf::RunMode::Clean => task_clean(&mut tasks, &conf),
        conf::RunMode::Edit => task_edit(&mut tasks, &conf),
        _ => {}
    }
}
