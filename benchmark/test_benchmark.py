import copy
import numpy as np
import pytest
from scheduling import audit, decode, pox, lox, hypervolume, ShopSampling, ShopCrossover, ShopMutation
from run import ShopProblem, apex_problem, native


def synthetic(kind="PFSP"):
    ins = dict(id="synthetic",kind=kind,machines=2,jobs=[[[[0,2]],[[1,3]]],[[[0,4]],[[1,1]]]],
               serial_upper=10,operations=4,source={"url":"synthetic"})
    if kind == "FJSP":
        ins["jobs"][0][0].append([1,5])
        ins["serial_upper"] = 13
    return ins


def test_known_flowshop_and_validation():
    ins = synthetic()
    objectives, rows = decode(ins, [0,1])
    assert objectives == [7,12]
    assert audit(ins, rows, objectives)
    bad = copy.deepcopy(rows)
    bad[2][3] = 1
    with pytest.raises(AssertionError):
        audit(ins, bad, objectives)
    with pytest.raises(AssertionError, match="Incorrect MS/FT"):
        audit(ins, rows, [7,11])
    with pytest.raises(AssertionError, match="Missing operations"):
        audit(ins, rows[:-1], objectives)


def test_feasible_nonpermutation_is_rejected():
    ins = synthetic()
    rows = [[0,0,0,0,2],[1,0,0,2,6],[1,1,1,6,7],[0,1,1,7,10]]
    with pytest.raises(AssertionError, match="permutation"):
        audit(ins, rows, [10,17])


def test_operator_multiplicities_and_decoding():
    rng = np.random.default_rng(7)
    assert sorted(pox([0,1,0,2,1],[2,1,0,1,0],[0])) == [0,0,1,1,2]
    assert sorted(lox([0,1,2,3],[3,2,1,0],1,3)) == list(range(4))
    for kind in ["PFSP","JSP","FJSP"]:
        problem = ShopProblem(synthetic(kind))
        sample = ShopSampling()._do(problem,100,random_state=rng)
        children = ShopCrossover()._do(problem,np.stack([sample,sample[::-1]]),random_state=rng).reshape(-1,problem.n_var)
        children = ShopMutation()._do(problem,children,random_state=rng)
        for child in children:
            f, rows = decode(problem.instance,child)
            assert audit(problem.instance,rows,f)


def test_hypervolume_against_known_area():
    assert hypervolume([[2,8],[5,4]], [10,10], (1,1)) == pytest.approx(0.36)
    assert hypervolume([[2,8],[5,4],[8,9]], [10,10], (1,1)) == pytest.approx(0.36)


@pytest.mark.parametrize("method", ["F","T","B","G","T-B","T-G","B-G","T-B-G"])
def test_native_pfsp_hard_rule_and_objectives(method):
    ins = synthetic()
    result = native(ins, method, 100, 19)
    assert result["evaluations"] <= 100
    assert result["archive"]
    for entry in result["archive"]:
        assert audit(ins, entry["schedule"], entry["objectives"])
