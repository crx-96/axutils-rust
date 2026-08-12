use std::sync::Once;

static DEFAULT_DRIVERS: Once = Once::new();

pub(crate) fn install_default_drivers() {
    DEFAULT_DRIVERS.call_once(sqlx::any::install_default_drivers);
}
