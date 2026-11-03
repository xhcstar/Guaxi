//! FSRS（Free Spaced Repetition Scheduler）的紧凑实现。
//!
//! 为什么不引 crate：FSRS 的公式是公开且稳定的，而这里只需要「给定上一次的
//! 稳定度/难度与本次评分，算出下一次稳定度与间隔」。引一个依赖要过离线构建、
//! 版本解析与代理这几道门，收益不成比例；公式写在这里还能直接跑单元测试。
//!
//! 采用 FSRS-5 的默认权重与公式（19 个参数）：
//! - 难度 D ∈ [1,10]；稳定度 S ≥ 0.1（天）。
//! - R(t,S) = (1 + FACTOR·t/S)^DECAY，FACTOR=19/81，DECAY=-0.5。
//! - 评分 G：1=忘了，2=勉强，3=记得，4=轻松。

/// FSRS-5 默认权重。
pub const DEFAULT_W: [f64; 19] = [
    0.40255, 1.18385, 3.173, 15.69105, 7.1949, 0.5345, 1.4604, 0.0046, 1.54575, 0.1192,
    1.01925, 1.9395, 0.11, 0.29605, 2.2698, 0.2315, 2.9898, 0.51655, 0.6621,
];

const DECAY: f64 = -0.5;
const FACTOR: f64 = 19.0 / 81.0;
const MIN_STABILITY: f64 = 0.1;
const MAX_INTERVAL_DAYS: i64 = 36500;

#[derive(Debug, Clone, Copy)]
pub struct Schedule {
    pub stability: f64,
    pub difficulty: f64,
    /// 下次复习间隔（天），已夹到 [1, 36500]。
    pub interval_days: i64,
    /// 本次复习时点（上一次复习后 elapsed 天）的检索成功概率。
    pub retrievability: f64,
}

fn clamp_difficulty(d: f64) -> f64 {
    d.clamp(1.0, 10.0)
}

fn init_difficulty(rating: u8) -> f64 {
    let w = &DEFAULT_W;
    clamp_difficulty(w[4] - (w[5] * (rating as f64 - 1.0)).exp() + 1.0)
}

fn next_difficulty(d: f64, rating: u8) -> f64 {
    let w = &DEFAULT_W;
    let d0_easy = init_difficulty(4);
    clamp_difficulty(w[7] * d0_easy + (1.0 - w[7]) * (d + w[6] * (rating as f64 - 3.0)))
}

fn init_stability(rating: u8) -> f64 {
    DEFAULT_W[(rating.clamp(1, 4) - 1) as usize].max(MIN_STABILITY)
}

/// 检索成功概率：上次复习后经过 `elapsed_days` 天、稳定度 `stability`。
pub fn retrievability(stability: f64, elapsed_days: f64) -> f64 {
    if stability <= 0.0 {
        return 0.0;
    }
    let t = elapsed_days.max(0.0);
    (1.0 + FACTOR * t / stability).powf(DECAY).clamp(0.0, 1.0)
}

/// 复习后的稳定度。
fn next_stability_good(s: f64, d: f64, r: f64, rating: u8) -> f64 {
    let w = &DEFAULT_W;
    let hard_penalty = if rating == 2 { w[15] } else { 1.0 };
    let easy_bonus = if rating == 4 { w[16] } else { 1.0 };
    let next = s
        * (1.0
            + w[8].exp()
                * (11.0 - d)
                * s.powf(-w[9])
                * ((w[10] * (1.0 - r)).exp() - 1.0)
                * hard_penalty
                * easy_bonus);
    next.max(MIN_STABILITY)
}

/// 遗忘后的稳定度。
fn next_stability_lapse(s: f64, d: f64, r: f64) -> f64 {
    let w = &DEFAULT_W;
    let next = w[11] * d.powf(-w[12]) * ((s + 1.0).powf(w[13]) - 1.0) * (w[14] * (1.0 - r)).exp();
    next.max(MIN_STABILITY)
}

/// 计算下一次排期。
///
/// `stability` / `difficulty` 为 None（或非正）表示这条还没被 FSRS 接管：
/// 首次复习直接按评分给出初始稳定度/难度。这样旧 SM-2 队列可以无痛迁移——
/// 第一次反馈之前按「新卡」处理，之后就一直走 FSRS。
pub fn schedule(
    stability: Option<f64>,
    difficulty: Option<f64>,
    rating: u8,
    elapsed_days: f64,
) -> Schedule {
    let rating = rating.clamp(1, 4);
    let prev_s = stability.filter(|s| *s > 0.0 && s.is_finite());
    let prev_d = difficulty.filter(|d| *d > 0.0 && d.is_finite());

    let (s, d, r) = match (prev_s, prev_d) {
        (Some(s), Some(d)) => {
            let r = retrievability(s, elapsed_days);
            let next_d = next_difficulty(d, rating);
            let next_s = if rating == 1 {
                next_stability_lapse(s, d, r)
            } else {
                next_stability_good(s, d, r, rating)
            };
            (next_s, next_d, r)
        }
        _ => {
            let d = init_difficulty(rating);
            let s = init_stability(rating);
            // 新卡没有「上次复习」，检索概率按 0 天算，R=1。
            (s, d, 1.0)
        }
    };

    Schedule {
        stability: s.max(MIN_STABILITY),
        difficulty: d,
        interval_days: (s.round() as i64).clamp(1, MAX_INTERVAL_DAYS),
        retrievability: r,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retrievability_decays() {
        let r0 = retrievability(10.0, 0.0);
        let r5 = retrievability(10.0, 5.0);
        let r20 = retrievability(10.0, 20.0);
        assert!(r0 > r5 && r5 > r20);
        assert!((r0 - 1.0).abs() < 1e-9);
    }

    #[test]
    fn forgetting_shortens_stability() {
        let base = schedule(Some(10.0), Some(5.0), 3, 10.0);
        let lapse = schedule(Some(10.0), Some(5.0), 1, 10.0);
        assert!(lapse.stability < base.stability);
        assert!(lapse.interval_days < base.interval_days.max(lapse.interval_days));
    }

    #[test]
    fn easy_interval_grows_faster_than_good() {
        let good = schedule(Some(10.0), Some(5.0), 3, 10.0);
        let easy = schedule(Some(10.0), Some(5.0), 4, 10.0);
        assert!(easy.interval_days >= good.interval_days);
    }

    #[test]
    fn new_card_gets_initial_state() {
        let s = schedule(None, None, 3, 0.0);
        assert!(s.stability > 0.0);
        assert!(s.difficulty >= 1.0 && s.difficulty <= 10.0);
        assert!(s.interval_days >= 1);
    }
}
