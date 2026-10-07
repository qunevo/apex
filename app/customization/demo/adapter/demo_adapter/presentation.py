"""Bounded source evidence for middleware views; the full report stays local."""


def source_summary(report):
    provenance = report["provenance"]
    clock = report["time_basis"]
    notices = [dict(code="DEMO_SNAPSHOT", message=(
        f"Frozen demo snapshot: {clock['as_of']} ({clock['timezone']}). "
        "Use this for today/now and overdue-at-snapshot questions, never the host/chat date. "
        "Predicted lateness uses validated completion minus due date. Do not shift source dates."
    ))]
    notices += [dict(code=str(warning["code"])[:80], message=str(warning["message"])[:500],
                    **({"entity": str(warning["id"])[:160]} if "id" in warning else {}))
               for warning in report["warnings"][:19]]
    return dict(
        sources=[dict(label="MES", revision=str(provenance["mes_revision"]), sha256=provenance["mes_sha256"]),
                 dict(label="Excel", sha256=provenance["workbook_sha256"])],
        counts={**report["counts"], "operations_without_excel": len(report["operations_without_excel"])},
        notices=notices, notice_count=1 + len(report["warnings"]),
    )
