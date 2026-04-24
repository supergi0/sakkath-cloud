use governor::middleware::NoOpMiddleware;
use std::sync::Arc;
use tower_governor::{
    GovernorLayer, governor::GovernorConfigBuilder, key_extractor::PeerIpKeyExtractor,
};

pub fn api_rate_limit_layer() -> GovernorLayer<PeerIpKeyExtractor, NoOpMiddleware> {
    GovernorLayer {
        config: Arc::new(
            GovernorConfigBuilder::default()
                .key_extractor(PeerIpKeyExtractor)
                .per_millisecond(30)
                .burst_size(2000)
                .finish()
                .unwrap(),
        ),
    }
}

pub fn ui_rate_limit_layer() -> GovernorLayer<PeerIpKeyExtractor, NoOpMiddleware> {
    GovernorLayer {
        config: Arc::new(
            GovernorConfigBuilder::default()
                .key_extractor(PeerIpKeyExtractor)
                .per_millisecond(6)
                .burst_size(10000)
                .finish()
                .unwrap(),
        ),
    }
}

pub fn telemetry_rate_limit_layer() -> GovernorLayer<PeerIpKeyExtractor, NoOpMiddleware> {
    GovernorLayer {
        config: Arc::new(
            GovernorConfigBuilder::default()
                .key_extractor(PeerIpKeyExtractor)
                .per_millisecond(600)
                .burst_size(100)
                .finish()
                .unwrap(),
        ),
    }
}
