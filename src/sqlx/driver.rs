use std::sync::Once;

use sqlx::any;

static DEFAULT_DRIVERS: Once = Once::new();

pub(crate) fn install_default_drivers() {
    DEFAULT_DRIVERS.call_once(any::install_default_drivers);
}
