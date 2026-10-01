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
        field("due", "Ship due", "datetime"),
        field("priority", "Priority", choices=["Normal", "High", "Urgent"]),
        field("status", "Status", editable=False), field("note", "Planner note", required=False),
    ]),
    "lots": table("Production lots", "Released lots with traceable work in progress", [
        field("order_id", "Order", ref="orders", editable=False),
        field("item_id", "Article", ref="items", editable=False),
        field("quantity", "Quantity", "integer", editable=False),
        field("release", "Release", "datetime", editable=False),
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
    ], create=False),
    "items": table("Articles", "Product definitions; released work instructions retain their original values", [
        field("name", "Description"), field("variant", "Variant", choices=["Distributor", "Regulator", "Sensor"]),
        field("size", "Body size", choices=["Compact", "Standard", "Large"]),
        field("material", "Body material", choices=["Aluminium", "Stainless steel"]),
        field("lot_size", "Standard lot", "integer"), field("routing_id", "Workplan", ref="routings"),
    ]),
    "routings": table("Workplans", "Fixed operation sequence per product family", [
        field("name", "Description"), field("steps", "Operation sequence", editable=False),
        field("revision", "Revision", editable=False),
    ], create=False),
    "machines": table("Machines & workplaces", "Equipment master and capability limits", [
        field("name", "Description"), field("group", "Group", editable=False),
        field("capability", "Capability", editable=False),
        field("calendar", "Calendar", ref="shifts"),
        field("permanently_unavailable", "Permanently unavailable", "boolean"),
        field("status", "Status at snapshot", choices=["Available", "Unavailable"], editable=False, required=False, computed=True),
        field("unavailable_from", "Unavailable from", "datetime", editable=False, required=False, computed=True),
        field("unavailable_until", "Unavailable until", "datetime", editable=False, required=False, computed=True),
        field("unavailability_reason", "Unavailability reason", editable=False, required=False, computed=True),
        field("note", "Note", required=False),
    ], create=False),
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
    "materials": table("Material stock", "Usable opening stock at the beginning of the planning period", [
        field("name", "Description"), field("unit", "Unit", choices=["pcs", "kg"]),
        field("stock", "Opening stock", "number"), field("location", "Storage location"),
        field("reorder_point", "Review below", "number"),
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
