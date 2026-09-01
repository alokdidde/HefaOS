use cu29::bundle_resources;
use cu29::prelude::{BundleContext, ComponentConfig, CuResult, ResourceBundle, ResourceManager};

#[derive(Debug)]
pub struct RunCounter;

impl RunCounter {
    pub const fn new() -> Self {
        Self
    }
}

pub struct RunBundle;

bundle_resources!(RunBundle: Counter);

impl ResourceBundle for RunBundle {
    fn build(
        bundle: BundleContext<Self>,
        _config: Option<&ComponentConfig>,
        manager: &mut ResourceManager,
    ) -> CuResult<()> {
        manager.add_owned(bundle.key(RunBundleId::Counter), RunCounter::new())
    }
}
