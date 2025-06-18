use video_bard::run;

fn main() {
    tracing_subscriber::fmt::init();

    run().expect("Didn't run");
}
