#![feature(thread_id_value)]

use simple_executor::{
	executor::*,
	timer::*,
};

use std::io::Write;
use std::time::Duration;

fn env_logger_init() {
    let env = env_logger::Env::default();
    env_logger::Builder::from_env(env).format(|buf, record| {
        let time = std::time::SystemTime::now();
        writeln!(buf, "[{} {:5} {} {} {:2}] {}",
            humantime::format_rfc3339_micros(time),
            record.level(),
            if let Some(s) = record.module_path_static() { s } else { "" },
            if let Some(v) = record.line() { v } else { 0 },
            std::thread::current().id().as_u64(),
            record.args())
    }).init();
}

#[test]
fn first_test() {
    env_logger_init();
    log::debug!("first_test:+");

    let (executor, spawner) = new_executor_and_spawner();
    
    const DELAY_MS: u64 = 500;
    
    // Spawn a task to print before and after waiting on a timer.
    spawner.spawn(async {
        println!("howdy!");
        // Wait for our timer future to complete after 500ms.
        TimerFuture::new(Duration::from_millis(DELAY_MS)).await;
        println!("done!");
    });
    
    // Drop the spawner so that our executor knows it is finished and won't
    // receive more incoming tasks to run.
    drop(spawner);
    
    let now = std::time::SystemTime::now();

    // Run the executor until the task queue is empty.
    // This will print "howdy!", pause, and then print "done!".
    executor.run();
    
    match now.elapsed() {
        Ok(elapsed) => assert!(elapsed >= Duration::from_millis(DELAY_MS)),
        Err(e) => panic!("{}", e),
    }

    log::debug!("first_test:-");
}
