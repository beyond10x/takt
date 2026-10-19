//! `check_id` over any text: no id it accepts leaves the cases directory; an accepted id is one
//! plain name under it.

#![no_main]

use std::path::{Component, Path};

use libfuzzer_sys::fuzz_target;
use takt::case::{CASES_DIR, check_id};

fuzz_target!(|data: &[u8]| {
    let Ok(id) = std::str::from_utf8(data) else {
        return;
    };
    if check_id(id).is_ok() {
        let components: Vec<Component<'_>> = Path::new(id).components().collect();
        assert_eq!(components, [Component::Normal(id.as_ref())], "{id:?}");
        let cases = Path::new("/srv/repo").join(CASES_DIR);
        assert_eq!(cases.join(id).parent(), Some(cases.as_path()), "{id:?}");
        assert!(!id.contains('\0'), "{id:?}");
    }
});
