"""Small deliberately synthetic source records and minimal OOXML parser fixtures."""
from copy import deepcopy
from io import BytesIO
from xml.sax.saxutils import escape
from zipfile import ZipFile

SKILLS = ("Setup", "Mechanical", "Precision", "Electrical", "Calibration", "Testing")
HEADERS = ["Operation ID", "Lot", "Order", "Qty (pcs)", "Workplace", "Sequence", "Person",
           "Planned start", "Setup (min)", "Run (min)", "Fixed", "Attendance (min)", "Planner note"]


def plan_row(ident="OP1", *, fixed="No", start="2026-10-05T10:30", machine="M1", person="P1"):
    return [ident, "LOT1", "ORDER1", 10, machine, 1, person, start, 10, 20, fixed, 10, "Synthetic"]


def workbook(rows=None, *, headers=None, qualifications=None, extra_row=None, formula=False):
    """Construct tiny ZIP/XML test inputs, including deliberately stale dimensions."""
    dispatch = [headers or HEADERS] + (rows if rows is not None else [plan_row()])
    skills = [["Person", *SKILLS]] + (qualifications if qualifications is not None else [
        ["P1", "Yes", "Yes", "No", "No", "No", "No"],
        ["P2", "Yes", "Yes", "No", "No", "No", "No"]])
    stream = BytesIO()
    with ZipFile(stream, "w") as archive:
        archive.writestr("[Content_Types].xml", '<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/></Types>')
        archive.writestr("_rels/.rels", '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>')
        archive.writestr("xl/workbook.xml", '<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="Dispatch plan" sheetId="1" r:id="rId1"/><sheet name="Skills" sheetId="2" r:id="rId2"/></sheets></workbook>')
        archive.writestr("xl/_rels/workbook.xml.rels", '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">' + ''.join(f'<Relationship Id="rId{n}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet{n}.xml"/>' for n in (1, 2)) + '</Relationships>')
        for sheet, data in enumerate((dispatch, skills), 1):
            numbered = list(enumerate(data, 4))
            if sheet == 1 and extra_row:
                numbered.append((20, extra_row))
            xml_rows = []
            for row_number, row in numbered:
                cells = []
                for col, value in enumerate(row):
                    coordinate = f"{chr(65+col)}{row_number}"
                    if formula and sheet == 1 and row_number == 5 and col == 8:
                        cells.append(f'<c r="{coordinate}"><f>5+5</f><v>10</v></c>')
                    elif isinstance(value, (int, float)):
                        cells.append(f'<c r="{coordinate}"><v>{value}</v></c>')
                    elif value is not None:
                        cells.append(f'<c r="{coordinate}" t="inlineStr"><is><t>{escape(str(value))}</t></is></c>')
                xml_rows.append(f'<row r="{row_number}">' + ''.join(cells) + '</row>')
            archive.writestr(f"xl/worksheets/sheet{sheet}.xml", '<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><dimension ref="A1:N5"/><sheetData>' + ''.join(xml_rows) + '</sheetData></worksheet>')
    return stream.getvalue()


def snapshot():
    first = dict(id="OP1", lot_id="LOT1", sequence=10, name="Machining", group="CNC", skill="Setup",
                 status="Waiting", completed_quantity=0, scrap_quantity=0, resource_id="", person_id="",
                 actual_start="", actual_end="", machine_options=[
                     dict(resource_id="M1", setup_minutes=10, minutes_per_unit=2, attendance="Setup only"),
                     dict(resource_id="M2", setup_minutes=5, minutes_per_unit=1, attendance="Setup only")],
                 material_requirements=[dict(material_id="RAW", quantity_per_unit=1)])
    second = deepcopy(first)
    second.update(id="OP2", sequence=20, name="Finish", group="Assembly", skill="Mechanical", material_requirements=[])
    second["machine_options"] = [dict(resource_id="M2", setup_minutes=5, minutes_per_unit=1, attendance="Continuous")]
    records = dict(
        orders=[dict(id="ORDER1", due="2026-10-06T16:00", priority="Normal")],
        lots=[dict(id="LOT1", order_id="ORDER1", item_id="ITEM1", quantity=10, release="2026-10-05T06:00", quality_status="Released")],
        operations=[first, second],
        machines=[dict(id=m, calendar="EQUIPMENT", permanently_unavailable=False) for m in ("M1", "M2")],
        personnel=[dict(id=p, shift_id="DAY", attendance="Present") for p in ("P1", "P2")],
        shifts=[dict(id="EQUIPMENT", start="06:00", end="22:00", break_start="22:00", break_end="22:00"),
                dict(id="DAY", start="06:00", end="14:00", break_start="10:00", break_end="10:30")],
        materials=[dict(id="RAW", unit="pcs", stock=100, on_hand=100)], receipts=[], absences=[], downtime=[],
        material_issues=[], confirmations=[])
    return dict(factory=dict(plan_start="2026-10-05T06:00", as_of="2026-10-05T10:00",
                             horizon_end="2026-10-09T22:00", timezone="Europe/Berlin"), revision=1, records=records)
