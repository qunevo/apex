"""Explicit field contracts shared by the UI, API and seed validation."""


def field(key, label, kind="text", *, choices=None, ref=None, editable=True, required=True, computed=False):
    return dict(key=key, label=label, type=kind, choices=choices, ref=ref,
                editable=editable, required=required, computed=computed)


def table(label, description, columns, *, create=True):
    return dict(label=label, description=description, columns=[field("id", "ID", editable=False)] + columns,
                create=create)


CATALOG = {
    "orders": table("Customer orders", "Delivery commitments and released demand", [
        field("customer", "Customer"), field("item_id", "Article", ref="items", editable=False),
        field("quantity", "Quantity", "integer", editable=False),
        field("routing_id", "Released workplan", ref="routings", editable=False, required=False),
        field("due", "Ship due", "datetime"),
        field("priority", "Priority", choices=["Normal", "High", "Urgent"]),
        field("status", "Status", editable=False), field("note", "Planner note", required=False),
        field("good_quantity", "Finished good quantity", "integer", editable=False, required=False),
        field("scrap_quantity", "Total scrap", "integer", editable=False, required=False),
    ]),
    "lots": table("Production lots", "Released lots with traceable work in progress", [
        field("order_id", "Order", ref="orders", editable=False),
        field("item_id", "Article", ref="items", editable=False),
        field("quantity", "Quantity", "integer", editable=False),
        field("release", "Release", "datetime", editable=False),
        field("routing_id", "Workplan", ref="routings", editable=False, required=False),
        field("routing_revision", "Revision", editable=False, required=False),
        field("good_quantity", "Finished good quantity", "integer", editable=False, required=False),
        field("scrap_quantity", "Total scrap", "integer", editable=False, required=False),
        field("quality_status", "Quality status", choices=["Released", "Hold"], required=False),
        field("hold_reason", "Quality hold reason", required=False),
        field("status", "Status", editable=False), field("location", "Current location", editable=False),
        field("note", "Shopfloor note", required=False),
    ], create=False),
    "operations": table("Operations", "Work instructions and booked progress; sequencing is owned in Excel", [
        field("lot_id", "Lot", ref="lots", editable=False),
        field("sequence", "Step", "integer", editable=False), field("name", "Operation", editable=False),
        field("group", "Resource group", editable=False), field("skill", "Qualification", editable=False),
        field("setup_minutes", "Setup (min)", "number", editable=False),
        field("run_minutes", "Run (min)", "number", editable=False),
        field("status", "Status", editable=False),
        field("completed_quantity", "Good quantity", "integer", editable=False),
        field("resource_id", "Actual resource", required=False, editable=False),
        field("scrap_quantity", "Scrap quantity", "integer", editable=False, required=False),
        field("person_id", "Actual operator", ref="personnel", required=False, editable=False),
        field("actual_start", "Actual start", "datetime", required=False, editable=False),
        field("actual_end", "Actual finish", "datetime", required=False, editable=False),
        field("scrap_reason", "Scrap reason", required=False, editable=False),
        field("instruction", "Work instruction", required=False, editable=False),
        field("output", "Output / WIP state", required=False, editable=False),
        field("machine_options", "Released machine alternatives", "json", required=False, editable=False),
        field("material_requirements", "Released material requirements", "json", required=False, editable=False),
    ], create=False),
    "items": table("Articles", "Product definitions; released work instructions retain their original values", [
        field("name", "Description"), field("variant", "Variant", choices=["Distributor", "Regulator", "Sensor"]),
        field("size", "Body size", choices=["Compact", "Standard", "Large"]),
        field("material", "Body material", choices=["Aluminium", "Stainless steel"]),
        field("lot_size", "Standard lot", "integer"), field("routing_id", "Workplan", ref="routings"),
    ]),
    "routings": table("Workplans", "Draft, check and release revisions; released instructions are immutable", [
        field("name", "Description"), field("family", "Product family", choices=["Distributor", "Regulator", "Sensor"]),
        field("revision", "Revision"), field("status", "Status", choices=["Draft", "Released"], editable=False),
        field("steps", "Operation sequence", editable=False, required=False, computed=True),
    ]),
    "routing_steps": table("Workplan steps", "Ascending step numbers define whole-lot precedence", [
        field("routing_id", "Workplan revision", ref="routings", editable=False),
        field("sequence", "Step number", "integer"), field("name", "Operation"),
        field("group", "Resource group", choices=["CNC", "Deburr", "Wash", "Assembly", "Electronics", "Calibration", "Test"]),
        field("skill", "Required skill", choices=["Setup", "Mechanical", "Precision", "Electrical", "Calibration", "Testing"]),
        field("attendance", "Operator attendance", choices=["Setup only", "Continuous"]),
        field("instruction", "Work instruction"), field("output", "Output / WIP state"),
        field("active", "Active step", "boolean"),
    ]),
    "routing_modes": table("Machine alternatives", "Times per piece already include the selected body size and material", [
        field("step_id", "Workplan step", ref="routing_steps", editable=False),
        field("resource_id", "Machine / workplace", ref="machines"),
        field("size", "Body size", choices=["Compact", "Standard", "Large"]),
        field("material", "Body material", choices=["Aluminium", "Stainless steel"]),
        field("setup_minutes", "Setup (min)", "number"), field("minutes_per_unit", "Minutes per piece", "number"),
        field("active", "Active alternative", "boolean"),
    ]),
    "routing_materials": table("Step materials", "Component consumption is assigned to the operation that uses it", [
        field("step_id", "Workplan step", ref="routing_steps", editable=False),
        field("material_id", "Component", ref="materials"),
        field("body_material", "Applies to body material", choices=["Any", "Aluminium", "Stainless steel"]),
        field("quantity_per_unit", "Quantity per input piece", "number"), field("active", "Active requirement", "boolean"),
    ]),
    "item_routings": table("Article workplans", "Explicitly approved alternative workplans for each article", [
        field("item_id", "Article", ref="items", editable=False),
        field("routing_id", "Workplan revision", ref="routings", editable=False),
        field("active", "Allowed for new orders", "boolean"),
    ]),
    "machines": table("Machines & workplaces", "Equipment master and capability limits", [
        field("name", "Description"), field("group", "Group", editable=False),
        field("capability", "Capability"),
        field("calendar", "Calendar", ref="shifts"),
        field("permanently_unavailable", "Permanently unavailable", "boolean"),
        field("status", "Status at snapshot", choices=["Available", "Unavailable"], editable=False, required=False, computed=True),
        field("unavailable_from", "Unavailable from", "datetime", editable=False, required=False, computed=True),
        field("unavailable_until", "Unavailable until", "datetime", editable=False, required=False, computed=True),
        field("unavailability_reason", "Unavailability reason", editable=False, required=False, computed=True),
        field("note", "Note", required=False),
    ]),
    "personnel": table("Personnel", "Attendance is kept here; the qualification matrix is maintained in Excel", [
        field("name", "Name"), field("team", "Team", choices=["Machining", "Assembly", "Quality"]),
        field("shift_id", "Shift", ref="shifts"),
        field("attendance", "Attendance", choices=["Present", "Absent", "Training"]),
        field("note", "Note", required=False),
    ]),
    "shifts": table("Shift calendars", "Local plant time, Monday to Friday", [
        field("name", "Shift"), field("start", "Start", "time"), field("end", "End", "time"),
        field("break_start", "Break start", "time"), field("break_end", "Break end", "time"),
    ]),
    "absences": table("Personnel absences", "Dated exceptions to the assigned shift", [
        field("person_id", "Person", ref="personnel"), field("start", "From", "datetime"),
        field("end", "Until", "datetime"), field("reason", "Reason"),
        field("cancelled", "Cancel this period", "boolean", required=False),
    ]),
    "materials": table("Material stock", "Usable opening stock at the beginning of the planning period", [
        field("name", "Description"), field("unit", "Unit", choices=["pcs", "kg"]),
        field("stock", "Opening stock", "number"), field("location", "Storage location"),
        field("reorder_point", "Review below", "number"),
        field("on_hand", "On hand at snapshot", "number", computed=True, editable=False, required=False),
    ]),
    "receipts": table("Inbound deliveries", "Confirmed supply arrivals, not procurement proposals", [
        field("material_id", "Material", ref="materials"), field("quantity", "Quantity", "number"),
        field("available_at", "Available from", "datetime"),
        field("status", "Status", choices=["Confirmed", "Delayed", "Received"]),
        field("note", "Delivery note", required=False),
    ]),
    "downtime": table("Equipment unavailability", "Dated equipment blocks, including maintenance", [
        field("resource_id", "Machine / workplace", ref="machines"),
        field("start", "From", "datetime"), field("end", "Until", "datetime"),
        field("reason", "Reason"),
        field("cancelled", "Cancel this period", "boolean", required=False),
    ]),
    "confirmations": table("Production confirmations", "Append-only quantity and execution history", [
        field("operation_id", "Operation", ref="operations", editable=False),
        field("good_delta", "Good pieces", "integer", editable=False),
        field("scrap_delta", "Scrap pieces", "integer", editable=False),
        field("resource_id", "Resource", ref="machines", editable=False),
        field("person_id", "Operator", ref="personnel", required=False, editable=False),
        field("actual_start", "Actual start", "datetime", required=False, editable=False),
        field("actual_end", "Actual finish", "datetime", required=False, editable=False),
        field("recorded_at", "Booked at snapshot", "datetime", editable=False),
        field("note", "Scrap reason / source", required=False, editable=False),
    ], create=False),
    "material_issues": table("Material issues", "Automatic component issues for confirmed input pieces", [
        field("operation_id", "Operation", ref="operations", editable=False),
        field("confirmation_id", "Confirmation", ref="confirmations", editable=False),
        field("material_id", "Component", ref="materials", editable=False),
        field("quantity", "Issued quantity", "number", editable=False),
        field("at", "Issue time", "datetime", editable=False),
    ], create=False),
}

STEPS = {
    "machine": ("CNC machining", "CNC", "Setup", 2.4, 18),
    "deburr": ("Deburr & inspect", "Deburr", "Mechanical", .7, 4),
    "wash": ("Wash & dry", "Wash", "Mechanical", .35, 12),
    "assemble": ("Mechanical assembly", "Assembly", "Mechanical", 1.8, 8),
    "adjust": ("Valve adjustment", "Assembly", "Precision", 1.1, 6),
    "premount": ("Mechanical preassembly", "Assembly", "Mechanical", 1.1, 6),
    "sensor": ("Sensor installation", "Electronics", "Electrical", 1.2, 8),
    "calibrate": ("Sensor calibration", "Calibration", "Calibration", 1.3, 10),
    "finish": ("Final assembly", "Assembly", "Precision", .8, 5),
    "test": ("Final leak / function test", "Test", "Testing", .9, 10),
}
ROUTES = {
    "RT-D": ["machine", "deburr", "wash", "assemble", "test"],
    "RT-R": ["machine", "deburr", "wash", "assemble", "adjust", "test"],
    "RT-S": ["machine", "deburr", "wash", "premount", "sensor", "calibrate", "finish", "test"],
}
SKILLS = ["Setup", "Mechanical", "Precision", "Electrical", "Calibration", "Testing"]
