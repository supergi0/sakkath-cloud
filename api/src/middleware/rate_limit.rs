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
                .per_millisecond(300)
                .burst_size(200)
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
                .per_millisecond(60)
                .burst_size(1000)
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
                .per_millisecond(1000)
                .burst_size(60)
                .finish()
                .unwrap(),
        ),
    }
}
