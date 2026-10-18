use ts_rs::{Config, TS};
use wonderland_my_wonderland::*;

const HEADER: &str = "// Generated from Rust by examples/generate_bindings.rs. Do not edit.\n";

fn main() {
    let cfg = Config::default();
    let mut out = String::from(HEADER);
    macro_rules! emit { ($($t:ty),*) => { $(out.push_str("export ");out.push_str(&<$t>::decl(&cfg));out.push('\n');)* }; }
    emit!(
        AccountStatus,
        LoginCancelReason,
        GameRole,
        Account,
        AccountSnapshot,
        Scope,
        Stat,
        DailyStat,
        DistributionRange,
        DistributionConfig,
        Bucket,
        DistributionDay,
        DistributionData,
        Distribution,
        Trend,
        StageInfo,
        Stage,
        Overview,
        MetricGroup,
        Comment,
        Comments,
        Detail,
        Series
    );
    out = out
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ui/src/types.generated.ts");
    if std::env::args().any(|a| a == "--check") {
        assert_eq!(
            std::fs::read_to_string(path).expect("generated file"),
            out,
            "Bindings drift: regenerate bindings"
        );
    } else {
        std::fs::write(path, out).unwrap();
    }
}
