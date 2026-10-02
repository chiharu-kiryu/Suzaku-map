#!/usr/bin/env python3
"""Run: python3 example.py /absolute/new-pack.json /path/to/suzaku_tool"""
import argparse
from suzaku_pack import PackBuilder


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output")
    parser.add_argument("tool")
    args = parser.parse_args()
    pack = PackBuilder(
        identifier="example.zh-hans.garden", language="zh-Hans", version="1.0.0",
        name="园艺 · 育苗", description="作者编写的育苗词句示例，不含个人资料。",
        topics=["gardening", "home"], authors=["Example author"], license="MIT",
    )
    pack.reading("yu'miao'pan", "育苗盘")
    pack.reading("pen'wu'qi", "喷雾器")
    pack.continuation("育苗盘", "育苗盘放在窗边了。", "育苗盘需要保持湿润。")
    pack.continuation("喷雾器", "喷雾器已经装好水了。")
    print(pack.build(args.output, tool=args.tool))


if __name__ == "__main__":
    main()
