import sys
import os
import json
from models import User, load_users
from utils import format_name, validate_email

def main():
    users = load_users("users.json")
    for user in users:
        name = format_name(user.name)
        if validate_email(user.email):
            print(f"Valid user: {name}")
        else:
            print(f"Invalid email for {name}")

if __name__ == "__main__":
    main()
