"""Discrete representations, independent schedule validation and Pareto metrics."""
from __future__ import annotations

import bisect
import numpy as np
from pymoo.core.sampling import Sampling
from pymoo.core.crossover import Crossover
from pymoo.core.mutation import Mutation


def flat_operations(instance):
    return [(j, k, alternatives) for j, job in enumerate(instance["jobs"])
            for k, alternatives in enumerate(job)]


def decode(instance, genome):
    jobs = instance["jobs"]
    n, m = len(jobs), instance["machines"]
    ready = [0]*n
    schedule = []
    if instance["kind"] == "PFSP":
        assert sorted(map(int, genome)) == list(range(n))
        machines = [0]*m
        for j in map(int, genome):
            for k in range(m):
                start = max(ready[j], machines[k])
                end = start + jobs[j][k][0][1]
                schedule.append([j, k, k, start, end])
                ready[j] = machines[k] = end
    else:
        operations = sum(map(len, jobs))
        seq = list(map(int, genome[:operations]))
        assert [seq.count(j) for j in range(n)] == list(map(len, jobs))
        offsets = np.cumsum([0] + list(map(len, jobs))).tolist()
        next_op = [0]*n
        occupied = [[] for _ in range(m)]
        for j in seq:
            k = next_op[j]
            mode = int(genome[operations + offsets[j] + k]) if instance["kind"] == "FJSP" else 0
            machine, duration = jobs[j][k][mode]
            start = ready[j]
            for left, right in occupied[machine]:
                if start + duration <= left:
                    break
                start = max(start, right)
            end = start + duration
            bisect.insort(occupied[machine], (start, end))
            schedule.append([j, k, machine, start, end])
            ready[j] = end
            next_op[j] += 1
    return [max(ready), sum(ready)], schedule


def audit(instance, schedule, objectives):
    """Check exported intervals without relying on either scheduling decoder."""
    expected = {(j, k): alternatives for j, job in enumerate(instance["jobs"])
                for k, alternatives in enumerate(job)}
    seen, machines = {}, [[] for _ in range(instance["machines"])]
    for row in schedule:
        assert len(row) == 5 and all(isinstance(x, int) for x in row), "Noninteger schedule"
        j, k, machine, start, end = row
        assert (j, k) in expected and (j, k) not in seen, "Operation coverage"
        assert start >= 0 and end > start, "Time interval"
        assert [machine, end-start] in expected[j, k], "Machine/duration mismatch"
        seen[j, k] = (start, end)
        machines[machine].append((start, end, j, k))
    assert seen.keys() == expected.keys(), "Missing operations"
    completion = []
    for j, job in enumerate(instance["jobs"]):
        for k in range(1, len(job)):
            assert seen[j, k][0] >= seen[j, k-1][1], "Job precedence"
        completion.append(seen[j, len(job)-1][1])
    orders = []
    for rows in machines:
        rows.sort()
        assert all(a[1] <= b[0] for a, b in zip(rows, rows[1:])), "Machine overlap"
        orders.append([row[2] for row in rows])
    if instance["kind"] == "PFSP":
        assert all(order == orders[0] for order in orders), "PFSP permutation violation"
    assert objectives == [max(completion), sum(completion)], "Incorrect MS/FT"
    return True


def pox(a, b, chosen):
    keep = set(chosen)
    tail = iter(x for x in b if x not in keep)
    return np.array([x if x in keep else next(tail) for x in a], dtype=int)


def lox(a, b, left, right):
    segment = set(a[left:right])
    tail = iter(x for x in b if x not in segment)
    return np.array([a[i] if left <= i < right else next(tail) for i in range(len(a))], dtype=int)


class ShopSampling(Sampling):
    def _do(self, problem, n_samples, random_state=None, **kwargs):
        rng, ins = random_state, problem.instance
        n = len(ins["jobs"])
        seq = list(range(n)) if ins["kind"] == "PFSP" else [j for j, job in enumerate(ins["jobs"]) for _ in job]
        rows = []
        for _ in range(n_samples):
            row = rng.permutation(seq).tolist()
            if ins["kind"] == "FJSP":
                row += [int(rng.integers(len(op))) for job in ins["jobs"] for op in job]
            rows.append(row)
        return np.array(rows, dtype=int)


class ShopCrossover(Crossover):
    def __init__(self):
        # Retain pymoo's generic crossover probability, 0.9 per mating.
        super().__init__(2, 2)

    def _do(self, problem, X, random_state=None, **kwargs):
        rng, ins = random_state, problem.instance
        length = len(ins["jobs"]) if ins["kind"] == "PFSP" else ins["operations"]
        out = X.copy()
        for i in range(X.shape[1]):
            a, b = X[0, i], X[1, i]
            if ins["kind"] == "PFSP":
                left, right = sorted(rng.choice(length+1, 2, replace=False))
                out[0, i, :length] = lox(a[:length], b[:length], left, right)
                out[1, i, :length] = lox(b[:length], a[:length], left, right)
            else:
                chosen = rng.choice(len(ins["jobs"]), int(rng.integers(1, len(ins["jobs"]))), replace=False)
                out[0, i, :length] = pox(a[:length], b[:length], chosen)
                out[1, i, :length] = pox(b[:length], a[:length], chosen)
                if ins["kind"] == "FJSP":
                    mask = rng.random(length) < 0.5
                    out[0, i, length:] = np.where(mask, a[length:], b[length:])
                    out[1, i, length:] = np.where(mask, b[length:], a[length:])
        return out


class ShopMutation(Mutation):
    def __init__(self):
        # Retain pymoo's generic permutation mutation application probability, 1.0.
        super().__init__()

    def _do(self, problem, X, random_state=None, **kwargs):
        rng, ins = random_state, problem.instance
        length = len(ins["jobs"]) if ins["kind"] == "PFSP" else ins["operations"]
        mutable = [i for i, (_, _, op) in enumerate(flat_operations(ins)) if len(op) > 1]
        out = X.copy()
        for row in out:
            a, b = rng.choice(length, 2, replace=False)
            row[a], row[b] = row[b], row[a]
            if ins["kind"] == "FJSP" and mutable:
                i = int(rng.choice(mutable))
                choices = len(flat_operations(ins)[i][2])
                old = int(row[length+i])
                row[length+i] = (old + int(rng.integers(1, choices))) % choices
        return out


def nondominated(rows):
    points = sorted({tuple(row) for row in rows})
    out, best_y = [], float("inf")
    for x, y in points:
        if y < best_y:
            out.append([x, y])
            best_y = y
    return out


def hypervolume(points, scale, ref=(1.1, 1.1)):
    best_y, area = ref[1], 0.0
    for x, y in nondominated(points):
        x, y = x/scale[0], y/scale[1]
        assert 0 <= x < ref[0] and 0 <= y < ref[1]
        if y < best_y:
            area += (ref[0]-x)*(best_y-y)
            best_y = y
    return area
