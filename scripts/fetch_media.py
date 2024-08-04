import json
from typing import List
import requests
import os
import argparse
from urllib.parse import quote
from pydantic import BaseModel

PALWORLD_WIKI_URL = "https://palworld.fandom.com/api.php?action=query&prop=imageinfo&iiprop=url&titles=File:"


def get_image_url(pal_name):
    return f"{PALWORLD_WIKI_URL}{quote(pal_name)}.png&format=json"


class Image(BaseModel):
    url: str
    description_url: str
    menu: bool = False


def load_json(file_path):
    with open(file_path, "r", encoding="utf-8") as file:
        return json.load(file)


def get_image_urls(pal_name: str) -> List[Image]:
    url = get_image_url(pal_name)
    print(f"Fetching: {url}")
    response = requests.get(url, timeout=10)
    data = response.json()
    images = []

    pages = data["query"]["pages"]
    page_id = list(pages.keys())[0]

    if "imageinfo" in pages[page_id]:
        img_url = pages[page_id]["imageinfo"][0]["url"]
        img_description_url = pages[page_id]["imageinfo"][0]["descriptionurl"]
        img = Image(url=img_url, description_url=img_description_url)
        images.append(img)
        print(f"Found: {img.model_dump()}")
        # return (pages[page_id]['imageinfo'][0]['url'], pages[page_id]['imageinfo'][0]['descriptionurl'])

    url = get_image_url(f"{pal_name} menu")
    print(f"Fetching: {url}")
    response = requests.get(url, timeout=10)
    data = response.json()

    pages = data["query"]["pages"]
    page_id = list(pages.keys())[0]

    if "imageinfo" in pages[page_id]:
        img_url = pages[page_id]["imageinfo"][0]["url"]
        img_description_url = pages[page_id]["imageinfo"][0]["descriptionurl"]
        img = Image(url=img_url, description_url=img_description_url, menu=True)
        images.append(img)
        print(f"Found: {img.model_dump()}")

    return images


def download_image(url, filename):
    response = requests.get(url, timeout=10)
    if response.status_code == 200:
        with open(filename, "wb") as file:
            file.write(response.content)
        print(f"Downloaded: {filename}")
    else:
        print(f"Failed to download: {filename}")


def to_snake_case(name):
    return name.lower().replace(" ", "_").replace("-", "_")


def main(output_dir):
    pals_data = load_json("pals.json")

    if not os.path.exists(output_dir):
        os.makedirs(output_dir)

    for internal_name, pal_name in pals_data.items():
        images = get_image_urls(pal_name)
        if len(images) > 0:
            # image_url, image_description_url = image_info
            for image in images:
                file_extension = os.path.splitext(image.description_url)[1]
                snake_case_name = to_snake_case(
                    f"{pal_name} menu" if image.menu else pal_name
                )
                filename = os.path.join(
                    output_dir, f"{snake_case_name}{file_extension}"
                )
                download_image(image.url, filename)
        else:
            print(f"Error: No images found for {pal_name} ({internal_name})")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Download Palworld images")
    parser.add_argument(
        "-o",
        "--output",
        default="images",
        help="Output directory for downloaded images",
    )
    args = parser.parse_args()

    main(args.output)
