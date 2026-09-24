import math

from prepare_figure_data import at_checkpoint, box_stats, trace_states


def test_tukey_whiskers_outlier_and_sample_variance():
    result = box_stats([0, 1, 2, 3, 100])
    assert (result["q1"], result["median"], result["q3"]) == (1, 2, 3)
    assert (result["whisker_low"], result["whisker_high"], result["outlier_count"]) == (0, 3, 1)
    assert math.isclose(result["sample_variance"], sum((v - 21.2) ** 2 for v in [0, 1, 2, 3, 100]) / 4)


def test_zero_variance_singleton_and_empty_are_different():
    assert box_stats([7, 7, 7])["sample_variance"] == 0
    assert box_stats([7])["sample_variance"] is None
    assert box_stats([])["median"] is None
    assert box_stats([7])["whisker_low"] == 7


def test_linear_quantiles_for_even_sample():
    result = box_stats([0, 2, 4, 6])
    assert (result["q1"], result["median"], result["q3"]) == (1.5, 3, 4.5)


def test_convergence_uses_only_observed_candidates_and_carries_final():
    trace = [dict(evaluation=1, seconds=.1), dict(evaluation=2, seconds=.2, objectives=[4, 4]), dict(evaluation=3, seconds=.3, objectives=[2, 6])]
    # Two rectangles: (1.1-.4)*(1.1-.4) + (.4-.2)*(1.1-.6) = .59.
    states = trace_states(trace, [10, 10], dict(hv=.59, ms=2, ft=4))
    assert abs(states[-1]["hv_deficit_pct"]) < 1e-12
    assert at_checkpoint(states, "seconds", .05, .4) == (None, "no_observation_yet")
    assert at_checkpoint(states, "evaluations", 1, .4)[1] == "no_feasible_candidate_yet"
    value, status = at_checkpoint(states, "seconds", .25, .4)
    assert status == "observed" and value["makespan_gap_pct"] == 100
    value, status = at_checkpoint(states, "evaluations", 1000, .4)
    assert status == "final_value_carried" and value["evaluations"] == 3
