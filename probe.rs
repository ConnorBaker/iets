// What a thread CPU clock charges a thread that sleeps, as iets's
// profile::tests::a_wait_is_not_cpu_time measures it. No dependencies.
use std::time::{Duration, Instant};

#[cfg(windows)]
mod clk {
    #[repr(C)]
    #[derive(Default, Clone, Copy)]
    struct FileTime { lo: u32, hi: u32 }
    extern "system" {
        fn GetCurrentThread() -> isize;
        fn GetThreadTimes(h: isize, c: *mut FileTime, e: *mut FileTime, k: *mut FileTime, u: *mut FileTime) -> i32;
        fn QueryThreadCycleTime(h: isize, cycles: *mut u64) -> i32;
    }
    /// GetThreadTimes kernel + user, in µs (what iets uses).
    pub fn cpu_us() -> u64 {
        let (mut c, mut e, mut k, mut u) = Default::default();
        unsafe { GetThreadTimes(GetCurrentThread(), &mut c, &mut e, &mut k, &mut u) };
        let t = |f: FileTime| (f.hi as u64) << 32 | f.lo as u64;
        (t(k) + t(u)) / 10
    }
    pub fn cycles() -> u64 {
        let mut n = 0;
        unsafe { QueryThreadCycleTime(GetCurrentThread(), &mut n) };
        n
    }
}

#[cfg(unix)]
mod clk {
    #[repr(C)]
    struct Ts { s: i64, ns: i64 }
    extern "C" { fn clock_gettime(id: i32, ts: *mut Ts) -> i32; }
    #[cfg(target_os = "linux")]
    const THREAD: i32 = 3;
    #[cfg(target_os = "macos")]
    const THREAD: i32 = 16;
    pub fn cpu_us() -> u64 {
        let mut t = Ts { s: 0, ns: 0 };
        unsafe { clock_gettime(THREAD, &mut t) };
        t.s as u64 * 1_000_000 + t.ns as u64 / 1000
    }
    pub fn cycles() -> u64 { 0 }
}

fn spin_cpu(us: u64) {
    let s = clk::cpu_us();
    while clk::cpu_us() - s < us {}
}

fn main() {
    let n: usize = std::env::args().nth(1).map_or(1000, |a| a.parse().unwrap());
    let sleep = Duration::from_millis(50);
    // the clock's own step: spin until it moves, many times
    let mut steps = Vec::new();
    for _ in 0..50 {
        let a = clk::cpu_us();
        let mut b = a;
        while b == a { b = clk::cpu_us(); }
        steps.push(b - a);
    }
    steps.sort();
    println!("clock step µs: min {} median {} max {}", steps[0], steps[steps.len() / 2], steps[steps.len() - 1]);
    for (label, pre_spin_us) in [("plain", 0u64), ("after 5 ms of CPU", 5_000)] {
        let mut charged = Vec::with_capacity(n);
        let mut cyc = Vec::with_capacity(n);
        let mut slept = Vec::with_capacity(n);
        let mut over = Vec::new();
        for i in 0..n {
            spin_cpu(pre_spin_us);
            let c0 = clk::cpu_us();
            let y0 = clk::cycles();
            let t0 = Instant::now();
            std::thread::sleep(sleep);
            let w = t0.elapsed().as_micros() as u64;
            let c = clk::cpu_us() - c0;
            let y = clk::cycles() - y0;
            if c >= 10_000 { over.push((i, c, w)); }
            charged.push(c);
            cyc.push(y);
            slept.push(w);
        }
        let mut s = charged.clone();
        s.sort();
        let mut hist = std::collections::BTreeMap::new();
        for c in &charged { *hist.entry(c / 1000).or_insert(0) += 1; }
        let mut yc = cyc.clone();
        yc.sort();
        let ge_slept = charged.iter().zip(&slept).filter(|(c, w)| c >= w).count();
        println!("[{label}] n={n} charged µs: min {} median {} p99 {} max {}; >=10ms: {}; >= slept: {}",
            s[0], s[n / 2], s[n * 99 / 100], s[n - 1], over.len(), ge_slept);
        println!("[{label}] charged ms histogram: {:?}", hist);
        println!("[{label}] cycles: median {} p99 {} max {}", yc[n / 2], yc[n * 99 / 100], yc[n - 1]);
        let mut w = slept.clone();
        w.sort();
        println!("[{label}] slept µs: min {} median {} max {}", w[0], w[n / 2], w[n - 1]);
        for (i, c, w) in over.iter().take(20) {
            println!("[{label}]   #{i}: charged {c} µs over a {w} µs sleep");
        }
    }
}
