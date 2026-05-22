//! Mapping between internal podcast category names (stored in
//! `categories.name` of podcast-service) and Apple Podcasts' two-level
//! taxonomy required for `<itunes:category>`.
//!
//! Source of truth for Apple side:
//! https://podcasters.apple.com/support/1691-apple-podcasts-categories
//!
//! Resolution rules (matches user spec):
//!   * If internal name matches a child (e.g. "Books"), we emit
//!     parent="Arts" with nested child="Books".
//!   * If internal name matches a parent that has children (e.g. "Arts"),
//!     we emit just parent="Arts" without a child.
//!   * If internal name matches a parent that has no children
//!     ("Government", "History", "Technology", "True Crime"), we emit
//!     just that parent.
//!   * Otherwise — no match, returns None. Caller decides what to do
//!     (omit category or pick a default).
//!
//! Lookup is case-insensitive on the internal side.

#[derive(Debug, Clone, Copy)]
pub struct AppleCategory {
    pub parent: &'static str,
    pub child: Option<&'static str>,
}

pub fn resolve(internal_name: &str) -> Option<AppleCategory> {
    let needle = internal_name.trim().to_ascii_lowercase();

    // Children first — most internal names will be leaf categories.
    for (parent, children) in TAXONOMY {
        for child in *children {
            if child.eq_ignore_ascii_case(&needle) {
                return Some(AppleCategory {
                    parent,
                    child: Some(child),
                });
            }
        }
    }

    // Then parents.
    for (parent, _) in TAXONOMY {
        if parent.eq_ignore_ascii_case(&needle) {
            return Some(AppleCategory {
                parent,
                child: None,
            });
        }
    }

    None
}

// Compile-time table — keeps memory layout tight and avoids HashMap init.
const TAXONOMY: &[(&str, &[&str])] = &[
    ("Arts", &[
        "Books", "Design", "Fashion & Beauty", "Food",
        "Performing Arts", "Visual Arts",
    ]),
    ("Business", &[
        "Careers", "Entrepreneurship", "Investing",
        "Management", "Marketing", "Non-Profit",
    ]),
    ("Comedy", &["Comedy Interviews", "Improv", "Stand-Up"]),
    ("Education", &[
        "Courses", "How To", "Language Learning", "Self-Improvement",
    ]),
    ("Fiction", &["Comedy Fiction", "Drama", "Science Fiction"]),
    ("Government", &[]),
    ("History", &[]),
    ("Health & Fitness", &[
        "Alternative Health", "Fitness", "Medicine", "Mental Health",
        "Nutrition", "Sexuality",
    ]),
    ("Kids & Family", &[
        "Education for Kids", "Parenting", "Pets & Animals", "Stories for Kids",
    ]),
    ("Leisure", &[
        "Animation & Manga", "Automotive", "Aviation", "Crafts",
        "Games", "Hobbies", "Home & Garden", "Video Games",
    ]),
    ("Music", &["Music Commentary", "Music History", "Music Interviews"]),
    ("News", &[
        "Business News", "Daily News", "Entertainment News",
        "News Commentary", "Politics", "Sports News", "Tech News",
    ]),
    ("Religion & Spirituality", &[
        "Buddhism", "Christianity", "Hinduism", "Islam", "Judaism",
        "Religion", "Spirituality",
    ]),
    ("Science", &[
        "Astronomy", "Chemistry", "Earth Sciences", "Life Sciences",
        "Mathematics", "Natural Sciences", "Nature", "Physics",
        "Social Sciences",
    ]),
    ("Society & Culture", &[
        "Documentary", "Personal Journals", "Philosophy",
        "Places & Travel", "Relationships",
    ]),
    ("Sports", &[
        "Baseball", "Basketball", "Cricket", "Fantasy Sports",
        "Football", "Golf", "Hockey", "Rugby", "Soccer", "Swimming",
        "Tennis", "Volleyball", "Wilderness", "Wrestling",
    ]),
    ("Technology", &[]),
    ("True Crime", &[]),
    ("TV & Film", &[
        "After Shows", "Film History", "Film Interviews",
        "Film Reviews", "TV Reviews",
    ]),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn child_match() {
        let c = resolve("Books").unwrap();
        assert_eq!(c.parent, "Arts");
        assert_eq!(c.child, Some("Books"));
    }

    #[test]
    fn child_match_case_insensitive() {
        let c = resolve("  bOOks ").unwrap();
        assert_eq!(c.parent, "Arts");
        assert_eq!(c.child, Some("Books"));
    }

    #[test]
    fn parent_match() {
        let c = resolve("Technology").unwrap();
        assert_eq!(c.parent, "Technology");
        assert_eq!(c.child, None);
    }

    #[test]
    fn unknown() {
        assert!(resolve("Nonexistent").is_none());
    }
}
