//! Break tips: what to do during a break. Deterministic rotation, user list replaces defaults.

pub const SHORT_TIPS: &[&str] = &[
    "Look at something 20 feet away for 20 seconds",
    "Blink slowly ten times to refresh your eyes",
    "Roll your shoulders back, then drop them",
    "Stand up and stretch your arms above your head",
    "Drink a glass of water",
    "Close your eyes and take five slow breaths",
    "Look out of a window and find the farthest thing you can see",
    "Rotate your wrists and stretch your fingers",
    "Gently turn your neck left, then right",
    "Stand up, shift your weight, and unclench your jaw",
    "Palm your eyes: rub your hands warm and rest them over closed eyes",
    "Walk to the next room and back",
    "Stretch your back: hands on hips, lean back slightly",
    "Relax your face, forehead and tongue",
    "Let your eyes follow a slow figure eight in the air",
    "Roll your ankles while standing",
];

pub const LONG_TIPS: &[&str] = &[
    "Walk for a few minutes, outside if you can",
    "Eat something or refill your water bottle",
    "Do a full stretch: neck, shoulders, back, legs",
    "Step outside for fresh air and daylight",
    "Make tea or coffee away from the screen",
    "Tidy one small thing on your desk",
    "Call or message someone just to say hello",
];

/// Tip for break number `cycle` (0-based). A non-empty `custom` list replaces the defaults.
pub fn pick(custom: &[String], cycle: u32, long: bool) -> String {
    if !custom.is_empty() {
        return custom[cycle as usize % custom.len()].clone();
    }
    let list = if long { LONG_TIPS } else { SHORT_TIPS };
    list[cycle as usize % list.len()].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotates_through_defaults_and_wraps() {
        let n = SHORT_TIPS.len() as u32;
        assert_eq!(pick(&[], 0, false), SHORT_TIPS[0]);
        assert_eq!(pick(&[], 1, false), SHORT_TIPS[1]);
        assert_eq!(pick(&[], n, false), SHORT_TIPS[0]);
        assert_eq!(pick(&[], 2, true), LONG_TIPS[2]);
    }

    #[test]
    fn custom_list_replaces_defaults() {
        let c = vec!["Pet the cat".to_string(), "Water plants".to_string()];
        assert_eq!(pick(&c, 0, false), "Pet the cat");
        assert_eq!(pick(&c, 3, true), "Water plants");
    }

    #[test]
    fn at_least_twenty_default_tips_all_clean() {
        assert!(SHORT_TIPS.len() + LONG_TIPS.len() >= 20);
        for t in SHORT_TIPS.iter().chain(LONG_TIPS) {
            assert!(!t.is_empty() && t.len() <= 120 && !t.contains('|') && !t.chars().any(|c| c.is_control()), "{t}");
            assert_eq!(&crate::engine::clean_tip(t), t);
        }
    }
}
