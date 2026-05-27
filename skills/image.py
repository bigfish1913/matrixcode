#!/usr/bin/env python3
"""
Image Search API Skill - Get actual image URLs from Unsplash, Pexels, and Pixabay
"""

import sys
import json
import urllib.parse
import requests
import os
from pathlib import Path
from typing import List, Dict, Optional

# 配置文件路径
CONFIG_FILE = os.path.join(os.path.dirname(__file__), "image-search-config.json")

def load_config() -> Dict[str, str]:
    """Load API keys from config file"""
    config_path = Path(CONFIG_FILE)

    if config_path.exists():
        with open(config_path, 'r', encoding='utf-8') as f:
            return json.load(f)

    # Return default values if config doesn't exist
    return {
        "UNSPLASH_ACCESS_KEY": "",
        "PEXELS_API_KEY": "",
        "PIXABAY_API_KEY": ""
    }

# Load configuration
config = load_config()
UNSPLASH_ACCESS_KEY = config.get("UNSPLASH_ACCESS_KEY", "") or "RzvcejpYO3pXXUK56qdzfoQo1JE02OPDt5q5PGcT9Sc"
PEXELS_API_KEY = config.get("PEXELS_API_KEY", "") or "RnF6Eqw387tpuM2Fm5KPRcOhfRwXBFJ9xEhpBR5Wk3PNQglMridZE6v6"
PIXABAY_API_KEY = config.get("PIXABAY_API_KEY", "") or "55941778-8871365726979f6cc9a8fba1d"

def search_unsplash(query: str, per_page: int = 5, page: int = 1) -> List[Dict]:
    """Search Unsplash API"""
    if UNSPLASH_ACCESS_KEY == "your-unsplash-access-key":
        print("⚠️  请先配置 Unsplash API Key")
        return []

    url = "https://api.unsplash.com/search/photos"
    headers = {"Authorization": f"Client-ID {UNSPLASH_ACCESS_KEY}"}
    params = {
        "query": query,
        "per_page": per_page,
        "page": page
    }

    try:
        response = requests.get(url, headers=headers, params=params)
        response.raise_for_status()
        data = response.json()

        results = []
        for photo in data.get("results", []):
            results.append({
                "id": photo["id"],
                "url": photo["urls"]["regular"],
                "full_url": photo["urls"]["full"],
                "thumb_url": photo["urls"]["thumb"],
                "description": photo.get("description") or photo.get("alt_description", "无描述"),
                "photographer": photo["user"]["name"],
                "photographer_url": photo["user"]["links"]["html"],
                "width": photo["width"],
                "height": photo["height"],
                "platform": "Unsplash"
            })
        return results
    except Exception as e:
        print(f"Unsplash API 错误: {e}")
        return []

def search_pexels(query: str, per_page: int = 5, page: int = 1) -> List[Dict]:
    """Search Pexels API"""
    if PEXELS_API_KEY == "your-pexels-api-key":
        print("⚠️  请先配置 Pexels API Key")
        return []

    url = f"https://api.pexels.com/v1/search"
    headers = {"Authorization": PEXELS_API_KEY}
    params = {
        "query": query,
        "per_page": per_page,
        "page": page,
        "locale": "zh-CN"
    }

    try:
        response = requests.get(url, headers=headers, params=params)
        response.raise_for_status()
        data = response.json()

        results = []
        for photo in data.get("photos", []):
            results.append({
                "id": photo["id"],
                "url": photo["src"]["large"],
                "full_url": photo["src"]["original"],
                "thumb_url": photo["src"]["medium"],
                "description": photo.get("alt", "无描述"),
                "photographer": photo["photographer"],
                "photographer_url": photo["photographer_url"],
                "width": photo["width"],
                "height": photo["height"],
                "platform": "Pexels"
            })
        return results
    except Exception as e:
        print(f"Pexels API 错误: {e}")
        return []

def search_pexels_videos(query: str, per_page: int = 5, page: int = 1) -> List[Dict]:
    """Search Pexels API for videos"""
    if PEXELS_API_KEY == "your-pexels-api-key":
        print("⚠️  请先配置 Pexels API Key")
        return []

    url = "https://api.pexels.com/videos/search"
    headers = {"Authorization": PEXELS_API_KEY}
    params = {
        "query": query,
        "per_page": per_page,
        "page": page,
        "locale": "zh-CN"
    }

    try:
        response = requests.get(url, headers=headers, params=params)
        response.raise_for_status()
        data = response.json()

        results = []
        for video in data.get("videos", []):
            video_files = video.get("video_files", [])
            # 按质量排序取最佳
            hd = next((f for f in video_files if f.get("quality") == "hd"), None)
            sd = next((f for f in video_files if f.get("quality") == "sd"), None)
            best = hd or sd or (video_files[0] if video_files else {})

            # 获取预览图
            video_pictures = video.get("video_pictures", [])
            thumb = video_pictures[0]["picture"] if video_pictures else ""

            results.append({
                "id": video["id"],
                "type": "video",
                "url": best.get("link", ""),
                "full_url": (hd or best).get("link", ""),
                "thumb_url": thumb,
                "small_url": (sd or best).get("link", ""),
                "description": video.get("url", "").split("/")[-2].replace("-", " ") if video.get("url") else "无描述",
                "photographer": video.get("user", {}).get("name", "未知"),
                "photographer_url": video.get("user", {}).get("url", ""),
                "width": best.get("width", 0),
                "height": best.get("height", 0),
                "duration": video.get("duration", 0),
                "platform": "Pexels",
                "page_url": video.get("url", "")
            })
        return results
    except Exception as e:
        print(f"Pexels Video API 错误: {e}")
        return []


def search_pixabay(query: str, per_page: int = 5, page: int = 1) -> List[Dict]:
    """Search Pixabay API for images"""
    if PIXABAY_API_KEY == "your-pixabay-api-key":
        print("⚠️  请先配置 Pixabay API Key")
        return []

    url = "https://pixabay.com/api/"
    params = {
        "key": PIXABAY_API_KEY,
        "q": query,
        "per_page": max(per_page, 3),
        "page": page,
        "image_type": "photo",
        "safesearch": "true"
    }

    try:
        response = requests.get(url, params=params)
        response.raise_for_status()
        data = response.json()

        results = []
        for photo in data.get("hits", []):
            results.append({
                "id": photo["id"],
                "url": photo["webformatURL"],
                "full_url": photo["largeImageURL"],
                "thumb_url": photo["previewURL"],
                "description": photo.get("tags", "无描述"),
                "photographer": photo.get("user", "未知"),
                "photographer_url": f"https://pixabay.com/users/{photo.get('user', '')}-{photo.get('user_id', '')}/",
                "width": photo["webformatWidth"],
                "height": photo["webformatHeight"],
                "platform": "Pixabay"
            })
        return results
    except Exception as e:
        print(f"Pixabay API 错误: {e}")
        return []


def search_pixabay_videos(query: str, per_page: int = 5, page: int = 1) -> List[Dict]:
    """Search Pixabay API for videos"""
    if PIXABAY_API_KEY == "your-pixabay-api-key":
        print("⚠️  请先配置 Pixabay API Key")
        return []

    url = "https://pixabay.com/api/videos/"
    params = {
        "key": PIXABAY_API_KEY,
        "q": query,
        "per_page": max(per_page, 3),
        "page": page,
        "safesearch": "true"
    }

    try:
        response = requests.get(url, params=params)
        response.raise_for_status()
        data = response.json()

        results = []
        for video in data.get("hits", []):
            videos_data = video.get("videos", {})
            large = videos_data.get("large", {})
            medium = videos_data.get("medium", {})
            small = videos_data.get("small", {})
            tiny = videos_data.get("tiny", {})

            results.append({
                "id": video["id"],
                "type": "video",
                "url": medium.get("url", ""),
                "full_url": large.get("url", ""),
                "thumb_url": tiny.get("url", ""),
                "small_url": small.get("url", ""),
                "description": video.get("tags", "无描述"),
                "photographer": video.get("user", "未知"),
                "photographer_url": f"https://pixabay.com/users/{video.get('user', '')}-{video.get('user_id', '')}/",
                "width": large.get("width", 0),
                "height": large.get("height", 0),
                "duration": video.get("duration", 0),
                "platform": "Pixabay",
                "page_url": video.get("pageURL", "")
            })
        return results
    except Exception as e:
        print(f"Pixabay Video API 错误: {e}")
        return []


def verify_url(url: str, timeout: int = 5) -> bool:
    """Verify a URL is accessible via HEAD request"""
    if not url:
        return False
    try:
        resp = requests.head(url, timeout=timeout, allow_redirects=True)
        return resp.status_code < 400
    except Exception:
        return False


def verify_and_filter_results(results: List[Dict]) -> List[Dict]:
    """Verify all results are accessible, filter out invalid ones"""
    if not results:
        return results

    valid_results = []
    for item in results:
        url = item.get("url", "")
        if verify_url(url):
            valid_results.append(item)
        else:
            print(f"  ⛔ 链接不可用，已跳过: {item.get('description', '')[:30]}")

    return valid_results

def display_results(results: List[Dict], show_json: bool = False):
    """Display search results (images and videos)"""
    if not results:
        print("未找到结果")
        return

    images = [r for r in results if r.get("type") != "video"]
    videos = [r for r in results if r.get("type") == "video"]

    if images:
        print(f"\n{'='*80}")
        print(f"📸 找到 {len(images)} 张图片")
        print(f"{'='*80}\n")

        for i, img in enumerate(images, 1):
            print(f"图片 {i} - {img['platform']}")
            print(f"{'-'*80}")
            print(f"描述: {img['description']}")
            print(f"摄影师: {img['photographer']}")
            print(f"尺寸: {img['width']} x {img['height']}")
            print(f"\n🔗 图片链接:")
            print(f"   预览图: {img['url']}")
            print(f"   原图:   {img['full_url']}")
            print(f"   缩略图: {img['thumb_url']}")
            print(f"\n摄影师主页: {img['photographer_url']}")
            print()

    if videos:
        print(f"\n{'='*80}")
        print(f"🎬 找到 {len(videos)} 个视频")
        print(f"{'='*80}\n")

        for i, vid in enumerate(videos, 1):
            print(f"视频 {i} - {vid['platform']}")
            print(f"{'-'*80}")
            print(f"描述: {vid['description']}")
            print(f"作者: {vid['photographer']}")
            print(f"尺寸: {vid['width']} x {vid['height']}")
            print(f"时长: {vid['duration']}s")
            print(f"\n🔗 视频链接:")
            print(f"   中等画质: {vid['url']}")
            print(f"   高画质:   {vid['full_url']}")
            print(f"   小尺寸:   {vid.get('small_url', '')}")
            print(f"   缩略图:   {vid['thumb_url']}")
            print(f"\n作者主页: {vid['photographer_url']}")
            if vid.get("page_url"):
                print(f"详情页: {vid['page_url']}")
            print()

    if show_json:
        print(f"\n{'='*80}")
        print("JSON 格式:")
        print(json.dumps(results, indent=2, ensure_ascii=False))


display_image_results = display_results

def save_results_to_file(results: List[Dict], filename: str = "image_search_results.json"):
    """Save results to JSON file"""
    with open(filename, 'w', encoding='utf-8') as f:
        json.dump(results, f, indent=2, ensure_ascii=False)
    print(f"\n✅ 结果已保存到: {filename}")

def main():
    """Main function"""
    if len(sys.argv) < 2:
        print("使用方法: image-search-api <搜索关键词> [选项]")
        print("\n示例:")
        print("  image-search-api 山脉风景")
        print("  image-search-api city night --platform unsplash")
        print("  image-search-api office workspace --count 10 --json")
        print("  image-search-api 海滩 --save results.json")
        print("  image-search-api nature --type video")
        print("  image-search-api ocean --type all")
        print("  image-search-api ocean --page 2  (获取第2页，换一批结果)")
        print("\n选项:")
        print("  --platform <name>   指定平台 (unsplash/pexels/pixabay/all)")
        print("  --count <number>    每个平台返回的结果数量 (默认: 5)")
        print("  --type <type>       搜索类型 (image/video/all, 默认: image)")
        print("  --page <number>     页码，用于获取更多结果 (默认: 1)")
        print("  --json              以 JSON 格式输出")
        print("  --save <file>       保存结果到文件")
        print("\n⚠️  首次使用前请配置 API Key:")
        print("   - Unsplash: https://unsplash.com/developers")
        print("   - Pexels: https://www.pexels.com/api/")
        print("   - Pixabay: https://pixabay.com/api/docs/")
        print("\n📝 注意: 视频搜索支持 Pexels 和 Pixabay 平台")
        sys.exit(1)

    # Parse arguments
    args = sys.argv[1:]
    query_parts = []
    platform = "all"
    count = 5
    page = 1
    search_type = "image"
    show_json = False
    save_file = None

    i = 0
    while i < len(args):
        if args[i] == "--platform" and i + 1 < len(args):
            platform = args[i + 1].lower()
            i += 1
        elif args[i] == "--count" and i + 1 < len(args):
            count = int(args[i + 1])
            i += 1
        elif args[i] == "--type" and i + 1 < len(args):
            search_type = args[i + 1].lower()
            i += 1
        elif args[i] == "--page" and i + 1 < len(args):
            page = int(args[i + 1])
            i += 1
        elif args[i] == "--json":
            show_json = True
        elif args[i] == "--save" and i + 1 < len(args):
            save_file = args[i + 1]
            i += 1
        else:
            query_parts.append(args[i])
        i += 1

    query = " ".join(query_parts)

    if not query:
        print("错误: 请提供搜索关键词")
        sys.exit(1)

    if page > 1:
        print(f"\n📄 第 {page} 页结果")

    max_retries = 3
    current_page = page
    all_results = []

    for attempt in range(max_retries):
        batch_results = []

        # Search images
        if search_type in ["image", "all"]:
            if platform in ["all", "unsplash"]:
                print(f"\n🔍 正在搜索 Unsplash 图片...")
                batch_results.extend(search_unsplash(query, count, current_page))

            if platform in ["all", "pexels"]:
                print(f"🔍 正在搜索 Pexels 图片...")
                batch_results.extend(search_pexels(query, count, current_page))

            if platform in ["all", "pixabay"]:
                print(f"🔍 正在搜索 Pixabay 图片...")
                batch_results.extend(search_pixabay(query, count, current_page))

        # Search videos (Pexels + Pixabay)
        if search_type in ["video", "all"]:
            if platform in ["all", "pexels"]:
                print(f"🔍 正在搜索 Pexels 视频...")
                batch_results.extend(search_pexels_videos(query, count, current_page))

            if platform in ["all", "pixabay"]:
                print(f"🔍 正在搜索 Pixabay 视频...")
                batch_results.extend(search_pixabay_videos(query, count, current_page))

        # Verify every result
        print(f"\n✅ 正在验证链接可用性...")
        valid_results = verify_and_filter_results(batch_results)
        all_results.extend(valid_results)

        if all_results:
            break
        else:
            if attempt < max_retries - 1:
                current_page += 1
                print(f"\n🔄 结果不可用，正在获取第 {current_page} 页新结果 (重试 {attempt + 2}/{max_retries})...")
            else:
                print("\n❌ 多次重试后仍无可用结果")

    # Display results
    display_results(all_results, show_json)

    # Save to file if requested
    if save_file:
        save_results_to_file(all_results, save_file)

if __name__ == "__main__":
    main()