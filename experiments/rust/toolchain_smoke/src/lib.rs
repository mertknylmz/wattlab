//! Toolchain learning example only; not part of the trainer or backend pipeline.

/// Average received power samples, preserving the difference between missing and zero.
/// The borrowed slice is read without taking ownership of the caller's measurements.
pub fn average_power_w(samples: &[Option<i16>]) -> Option<f64> {
    let (sum, count) = samples
        .iter()
        .flatten()
        .fold((0.0, 0_u64), |(sum, count), &watts| {
            (sum + f64::from(watts), count + 1)
        });

    if count == 0 {
        None
    } else {
        Some(sum / count as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::average_power_w;

    #[test]
    fn missing_samples_are_excluded_but_zero_is_a_measurement() {
        assert_eq!(average_power_w(&[Some(100), None, Some(0)]), Some(50.0));
    }

    #[test]
    fn no_measurements_produces_none() {
        assert_eq!(average_power_w(&[]), None);
        assert_eq!(average_power_w(&[None, None]), None);
    }

    #[test]
    fn signed_power_values_remain_signed() {
        assert_eq!(average_power_w(&[Some(-10), Some(10)]), Some(0.0));
    }
}
