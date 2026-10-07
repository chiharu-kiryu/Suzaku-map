// Added by Suzaku on 2026-10-07; see NOTICE-SUZAKU.md for provenance and scope.
// This exact module is also included by Suzaku's root integration tests.

use crate::dpi::PhysicalPosition;
use crate::event::WindowEvent;

// A button event already contains its event-time window-local coordinates. Deliver
// them even when equal to the cached position: a stationary pointer need not cause
// XI_Motion after the window moves. Do not replace them with a later pointer query.
// Keep this separate from CursorMoved: a queued release must not advance a drag
// again using coordinates from before an earlier motion moved the window.
// Cache updates finish before any application callback, without holding its lock.
pub(super) fn suzaku_button_events_with_position(
    event: WindowEvent,
    position: PhysicalPosition<f64>,
    cache_cursor: impl FnOnce((f64, f64)),
) -> [Option<WindowEvent>; 2] {
    let cursor_event = match &event {
        WindowEvent::MouseInput { device_id, .. } => {
            cache_cursor((position.x, position.y));
            Some(WindowEvent::X11MouseButtonPosition {
                device_id: *device_id,
                position,
            })
        }
        // XI2 button details 4..=7 retain the upstream wheel-only behavior.
        _ => None,
    };
    [cursor_event, Some(event)]
}

#[cfg(test)]
mod suzaku_button_position_tests {
    use super::*;
    use crate::event::{DeviceId, ElementState, MouseButton, MouseScrollDelta, TouchPhase};

    #[test]
    fn position_precedes_button_and_refreshes_cached_local_coordinates() {
        let device_id = DeviceId::dummy();
        let position = PhysicalPosition::new(914.0, 402.0);
        let button = WindowEvent::MouseInput {
            device_id,
            state: ElementState::Pressed,
            button: MouseButton::Left,
        };
        let mut cached = Some((914.0, 296.0));
        let events = suzaku_button_events_with_position(button.clone(), position, |position| {
            cached = Some(position);
        });

        assert_eq!(cached, Some((914.0, 402.0)));
        assert_eq!(
            events,
            [
                Some(WindowEvent::X11MouseButtonPosition {
                    device_id,
                    position
                }),
                Some(button)
            ]
        );
    }

    #[test]
    fn same_point_repeated_press_and_release_each_publish_coordinates() {
        let device_id = DeviceId::dummy();
        let position = PhysicalPosition::new(914.0, 402.0);
        let mut cached = Some((position.x, position.y));
        let mut updates = 0;
        for state in [
            ElementState::Pressed,
            ElementState::Released,
            ElementState::Pressed,
            ElementState::Released,
        ] {
            let button = WindowEvent::MouseInput {
                device_id,
                state,
                button: MouseButton::Left,
            };
            let events = suzaku_button_events_with_position(button.clone(), position, |position| {
                cached = Some(position);
                updates += 1;
            });
            assert_eq!(
                events,
                [
                    Some(WindowEvent::X11MouseButtonPosition {
                        device_id,
                        position
                    }),
                    Some(button)
                ]
            );
        }
        assert_eq!(updates, 4);
        assert_eq!(cached, Some((914.0, 402.0)));
    }

    #[test]
    fn position_keeps_event_device_button_state_and_signed_fractional_coordinates() {
        let device_id = DeviceId::dummy();
        for state in [ElementState::Pressed, ElementState::Released] {
            for button in [
                MouseButton::Left,
                MouseButton::Middle,
                MouseButton::Right,
                MouseButton::Back,
                MouseButton::Forward,
                MouseButton::Other(12),
            ] {
                let position = PhysicalPosition::new(-12.5, 3.25);
                let event = WindowEvent::MouseInput {
                    device_id,
                    state,
                    button,
                };
                let mut cached = None;
                let events =
                    suzaku_button_events_with_position(event.clone(), position, |position| {
                        cached = Some(position);
                    });
                assert_eq!(cached, Some((-12.5, 3.25)));
                assert_eq!(
                    events,
                    [
                        Some(WindowEvent::X11MouseButtonPosition {
                            device_id,
                            position
                        }),
                        Some(event)
                    ]
                );
            }
        }
    }

    #[test]
    fn queued_button_positions_preserve_order_without_motion_events() {
        let device_id = DeviceId::dummy();
        let samples = [
            (ElementState::Pressed, PhysicalPosition::new(12.0, 20.0)),
            (ElementState::Released, PhysicalPosition::new(42.0, 20.0)),
            (ElementState::Pressed, PhysicalPosition::new(80.0, 31.0)),
            (ElementState::Released, PhysicalPosition::new(-4.0, 16.0)),
        ];
        let mut delivered = Vec::new();
        let mut cached_positions = Vec::new();
        for (state, position) in samples {
            let button = WindowEvent::MouseInput {
                device_id,
                state,
                button: MouseButton::Left,
            };
            delivered.extend(
                suzaku_button_events_with_position(button, position, |position| {
                    cached_positions.push(position);
                })
                .into_iter()
                .flatten(),
            );
        }

        assert_eq!(delivered.len(), samples.len() * 2);
        for (index, (state, position)) in samples.into_iter().enumerate() {
            assert_eq!(cached_positions[index], (position.x, position.y));
            assert_eq!(
                delivered[index * 2],
                WindowEvent::X11MouseButtonPosition {
                    device_id,
                    position
                }
            );
            assert_eq!(
                delivered[index * 2 + 1],
                WindowEvent::MouseInput {
                    device_id,
                    state,
                    button: MouseButton::Left,
                }
            );
        }
        assert!(
            delivered
                .iter()
                .all(|event| !matches!(event, WindowEvent::CursorMoved { .. }))
        );
    }

    #[test]
    fn wheel_events_do_not_publish_coordinates_or_update_the_cache() {
        for delta in [
            MouseScrollDelta::LineDelta(0.0, 1.0),
            MouseScrollDelta::LineDelta(0.0, -1.0),
            MouseScrollDelta::LineDelta(1.0, 0.0),
            MouseScrollDelta::LineDelta(-1.0, 0.0),
        ] {
            let wheel = WindowEvent::MouseWheel {
                device_id: DeviceId::dummy(),
                delta,
                phase: TouchPhase::Moved,
            };
            let events = suzaku_button_events_with_position(
                wheel.clone(),
                PhysicalPosition::new(914.0, 402.0),
                |_| panic!("wheel-only events must not update the button-position cache"),
            );
            assert_eq!(events, [None, Some(wheel)]);
        }
    }
}
