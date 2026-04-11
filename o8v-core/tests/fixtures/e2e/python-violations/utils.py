import re
import os
import sys

def format_name(name):
    return name.strip().title()

def validate_email(email):
    pattern = r'^[a-zA-Z0-9_.+-]+@[a-zA-Z0-9-]+\.[a-zA-Z0-9-.]+$'
    return re.match(pattern, email) is not None

def read_file(path):
    with open(path) as f:
        return f.read()

def write_output(data, output_path):
    with open(output_path, "w") as f:
        f.write(data)

def describe(name, email, role, department, location, manager, start_date, end_date, notes):
    return f"{name} <{email}> [{role}] {department} @ {location} managed by {manager} from {start_date} to {end_date}: {notes}"
