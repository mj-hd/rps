use smol::future::yield_now;

pub async fn sleep_cycles(cycles: u16) {
    let mut remaining = cycles;

    loop {
        remaining -= 1;
        yield_now().await;
        if remaining == 0 {
            return;
        }
    }
}
