use tappet_derive::{interface, Parameters};

import!();

new_type!(IEconService);

impl_conversions!(@PostQueryBuilder -> @IEconService);
convert_with_endpoint!(@PostQueryBuilder -> @IEconService);