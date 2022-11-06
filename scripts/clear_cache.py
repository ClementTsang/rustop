#!/bin/python3

# A simple script to clean caches matching a PR ID.
#
# Expects the GitHub token in the environment variables.

import os
import json
import sys
from urllib.error import HTTPError, URLError

from urllib.request import Request, urlopen

URL = "https://api.github.com/repos/ClementTsang/bottom/actions/caches"


def cache_list_request(key):
    request = Request(URL, method="GET")
    request.add_header("Accept", "application/vnd.github+json")
    request.add_header("Authorization", "Bearer {}".format(key))
    return request


def delete_cache_request(key, id):
    request = Request("{}/{}".format(URL, id), method="DELETE")
    request.add_header("Accept", "application/vnd.github+json")
    request.add_header("Authorization", "Bearer {}".format(key))
    return request


def main():

    args = sys.argv
    env = os.environ

    key = env["GITHUB_TOKEN"]
    pr_id = args[1]
    ref = "refs/pull/{}/merge".format(pr_id)

    with urlopen(cache_list_request(key)) as response:
        response = json.load(response)
        caches = response["actions_caches"]
        for cache in caches:
            if cache["ref"] == ref:
                id = cache["id"]
                try:
                    print("Deleting ID {}...".format(id))
                    urlopen(delete_cache_request(key, id))
                except HTTPError as e:
                    print("HTTPError with delete, error code {}.".format(e.code))
                except URLError as _:
                    print("URLError with delete.")
                else:
                    print("Successfully deleted cache ID {}!".format(id))


if __name__ == "__main__":
    main()
