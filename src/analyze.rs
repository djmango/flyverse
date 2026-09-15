//! Behaviour analysis harness for the embodied fly.
//!
//! Runs the closed loop headless, records a trace, and reports what the fly
//! actually did. Every number is measured from that run; nothing is asserted.
//!
//! Why this exists: the flight layer is an engineered surrogate (see body.rs),
//! so the interesting question is not "did it fly" but "which behaviours are
//! caused by the connectome and which are produced by the surrogate loops".
//! The loom-versus-steering statistics below are the direct test of that: the
//! room reaches the brain only through the sensory channels in `World::sensors`
//! (odour at the antennae, optic flow, taste, and the looming-wall channel). If
//! a looming wall does not move the steering motor neurons, the fly cannot
//! avoid the wall, no matter how good the aerodynamics are.
//!
//! The implementation is split into submodules; this hub only declares them and
//! re-exports the public surface, so callers keep using `analyze::…` unchanged.

mod attractor;
mod flight;
mod mn_audit;
mod probe;
mod report;
mod run;
mod stats;
mod summary;
mod trace;
mod yaw;

pub use flight::flight_test;
pub use mn_audit::{mn_audit, AuditOptions};
pub use probe::{rotation_probe, ProbeOptions};
pub use run::run;
pub use trace::{Options, Sample};
pub use yaw::{yaw_probe, YawOptions};
