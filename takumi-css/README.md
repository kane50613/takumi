# takumi-css

<!-- cargo-rdme start -->

CSS parsing and computed-style layer for takumi.

Holds the (cold) CSS parsing, cascade, value types, and selector matching so
they can be compiled independently from the hot rendering paths in `takumi`.
Matching is generic over a [`matching::MatchableNode`] the caller implements,
keeping this crate free of any node/render dependency and the `selectors`
crate out of takumi's public API.

<!-- cargo-rdme end -->
