use log::info;
use std::{thread, time::Duration};
use xcap::Monitor;

fn main() {
    pretty_env_logger::init();

    let monitor = Monitor::from_point(100, 100).unwrap();

    let (video_recorder, sx) = monitor.video_recorder().unwrap();

    let handle = thread::spawn(move || {
        while let Ok(frame) = sx.recv() {
            println!("frame: {:?}", frame.width);
        }
        info!("frame receiver thread exited")
    });

    println!("start");
    video_recorder.start().unwrap();
    thread::sleep(Duration::from_secs(2));
    println!("pause");
    video_recorder.pause().unwrap();
    thread::sleep(Duration::from_secs(2));
    println!("start");
    video_recorder.start().unwrap();
    thread::sleep(Duration::from_secs(2));
    println!("stop");
    video_recorder.stop().unwrap(); // NOTE: currently it is expected the user to always call stop() manually
    // drop trait is not implemented yet
}
