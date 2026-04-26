import json
import os
import sys
from datetime import datetime

class User:
    def __init__(self, name, email):
        self.name = name
        self.email = email
        self.created = datetime.now()

    def greet(self):
        return f"Hello, {self.name}!"

def load_users(path):
    with open(path) as f:
        data = json.load(f)
    return [User(u["name"], u["email"]) for u in data]
