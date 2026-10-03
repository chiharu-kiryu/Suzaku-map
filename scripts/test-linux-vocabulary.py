#!/usr/bin/env python3
"""Offline bilingual vocabulary through XTest, real GTK/IBus and saved text.

Use test-linux-apps.sh vocabulary; never run against the personal desktop.
The shared harness enforces private display/bus/configuration and observes both
the real editor buffer and completed save. No companion-seeded draft is used.
"""
from collections import Counter
import importlib.util
import json
from pathlib import Path
import sys

sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("apps", Path(__file__).with_name("test-linux-apps.py"))
apps = importlib.util.module_from_spec(spec)
spec.loader.exec_module(apps)
assert apps.os.environ["SUZAKU_APP_QA_SUITE"] == "vocabulary"
IBus = apps.IBus

# Writing, daily conversation/devices and model-unavailable fallback vocabulary.
CASES = [
    ("en", "please acknowledge rece", "please acknowledge receipt",
     "please acknowledge receipt of this message."),
    ("en", "please keep me po", "please keep me posted",
     "please keep me posted on your progress."),
    ("en", "can we catch up tom", "can we catch up tomorrow", "can we catch up tomorrow?"),
    ("en", "i should've call", "i should've called", "i should've called earlier."),
    ("en", "is this available in a larg", "is this available in a larger",
     "is this available in a larger size?"),
    ("en", "the battery needs rech", "the battery needs recharging",
     "the battery needs recharging."),
    ("en", "could you speak more slo", "could you speak more slowly",
     "could you speak more slowly?"),
    ("en", "please resend the atta", "please resend the attachment",
     "please resend the attachment."),
    ("en", "the network is temp", "the network is temporarily",
     "the network is temporarily unavailable."),
    ("en", "where is the nearest rest", "where is the nearest restroom",
     "where is the nearest restroom?"),
    ("en", "are you available tom", "are you available tomorrow",
     "are you available tomorrow afternoon?"),
    ("en", "please grant me acc", "please grant me access",
     "please grant me access to this document."),
    ("en", "my flight has been del", "my flight has been delayed",
     "my flight has been delayed."),
    ("en", "does this contain dai", "does this contain dairy", "does this contain dairy?"),
    ("en", "would tomorrow mor", "would tomorrow morning", "would tomorrow morning work for you?"),
    ("en", "when does the subscription exp", "when does the subscription expire",
     "when does the subscription expire?"),
    ("en", "i was charged tw", "i was charged twice", "i was charged twice for this order."),
    ("en", "could you send me a rep", "could you send me a replacement",
     "could you send me a replacement?"),
    ("en", "what does this me", "what does this mean", "what does this mean in this context?"),
    ("en", "how do you pron", "how do you pronounce", "how do you pronounce this word?"),
    ("en", "please save a co", "please save a copy", "please save a copy before closing."),
    ("en", "please turn on the cap", "please turn on the captions", "please turn on the captions."),
    ("zh-Hans", "hui yi ji yao", "会议纪要", "会议纪要已经发到群里了。"),
    ("zh-Hans", "gai'qi", "改期", "改期后的时间我再确认一下。"),
    ("zh-Hans", "zao'an", "早安", "早安，今天也要加油。"),
    ("zh-Hans", "yi hui er", "一会儿", "一会儿见。"),
    ("zh-Hans", "gou wu che", "购物车", "购物车里的商品需要再确认一下。"),
    ("zh-Hans", "chong dian bao", "充电宝", "充电宝需要提前充电。"),
    ("zh-Hans", "qing shao deng", "请稍等", "请稍等，我查一下。"),
    ("zh-Hans", "mei shou dao", "没收到", "没收到，可以重新发一下吗？"),
    ("zh-Hans", "li xian mo shi", "离线模式", "离线模式下仍然可以输入。"),
    ("zh-Hans", "bao liu yuan wen", "保留原文", "保留原文，不要自动替换。"),
    ("zh-Hans", "hui yi lian jie", "会议链接", "会议链接已经发到群里了。"),
    ("zh-Hans", "fang wen quan xian", "访问权限", "访问权限不足，麻烦帮我开通。"),
    ("zh-Hans", "sui shen xing li", "随身行李", "随身行李有重量限制吗？"),
    ("zh-Hans", "gou wu qing dan", "购物清单", "购物清单我已经列好了。"),
    ("zh-Hans", "ming tian xia wu", "明天下午", "明天下午可以安排。"),
    ("zh-Hans", "zi dong xu fei", "自动续费", "自动续费可以关闭吗？"),
    ("zh-Hans", "chong fu kou kuan", "重复扣款", "重复扣款了，请帮我核对。"),
    ("zh-Hans", "dian zi fa piao", "电子发票", "电子发票请发到我的邮箱。"),
    ("zh-Hans", "fa yin", "发音", "发音可以再示范一下吗？"),
    ("zh-Hans", "ju ge li zi", "举个例子", "举个例子会更容易理解。"),
    ("zh-Hans", "ping mu jie tu", "屏幕截图", "屏幕截图稍后发给你。"),
    ("zh-Hans", "zi ti da xiao", "字体大小", "字体大小可以在设置里调整。"),
]

HOME_CASES = [
    ("en", "please close the win", "please close the window", "please close the window before leaving."),
    ("en", "could you lend me a scre", "could you lend me a screwdriver", "could you lend me a screwdriver?"),
    ("zh-Hans", "bei yong yao shi", "备用钥匙", "备用钥匙放在抽屉里。"),
    ("zh-Hans", "shui long tou lou shui", "水龙头漏水", "水龙头漏水了，需要预约维修。"),
]

ERRANDS_CASES = [
    ("en", "please print it on bo", "please print it on both", "please print it on both sides."),
    ("en", "i'd like to renew this bo", "i'd like to renew this book", "i'd like to renew this book."),
    ("zh-Hans", "xu jie tu shu", "续借图书", "续借图书可以在网上办理吗？"),
    ("zh-Hans", "qu jian ma", "取件码", "取件码还没有收到。"),
]
DAILY_CASES = [
    ("en", "that's really imp", "that's really impressive", "that's really impressive."),
    ("en", "i'm almost th", "i'm almost there", "i'm almost there."),
    ("en", "let's take a br", "let's take a break", "let's take a break."),
    ("zh-Hans", "zhen bu cuo", "真不错", "真不错，下次还想再来。"),
    ("zh-Hans", "dao jia le", "到家了", "到家了，给你报个平安。"),
    ("zh-Hans", "xiu xi yi xia", "休息一下", "休息一下，等会儿再继续。"),
]

DAILY_NEEDS_CASES = [
    ("en", "please make it mi", "please make it mild", "please make it mild."),
    ("en", "it's getting co", "it's getting cold", "it's getting cold outside."),
    ("en", "can we change the ti", "can we change the time", "can we change the time?"),
    ("zh-Hans", "shao tang", "少糖", "少糖就好，谢谢。"),
    ("zh-Hans", "dai san", "带伞", "带伞出门，免得淋雨。"),
    ("zh-Hans", "lin shi you shi", "临时有事", "临时有事，可能要晚一点。"),
]

PACK_CASES = [
    ("en", "please annotate this para", "please annotate this paragraph",
     "please annotate this paragraph before our discussion."),
    ("zh-Hans", "yue du pi zhu", "阅读批注", "阅读批注请写在页边空白处。"),
    ("en", "please pass me the col", "please pass me the colander",
     "please pass me the colander from the cupboard."),
    ("zh-Hans", "shi cai qing dan", "食材清单", "食材清单先按菜谱整理一下。"),
    ("en", "could you store our lugg", "could you store our luggage",
     "could you store our luggage until this afternoon?"),
    ("zh-Hans", "xing li ji cun", "行李寄存", "行李寄存可以到下午吗？"),
    ("en", "please update the project road", "please update the project roadmap",
     "please update the project roadmap before our next meeting."),
    ("zh-Hans", "xiang mu lu xian tu", "项目路线图", "项目路线图请在下次会议前更新。"),
]
# A focused diagnostic never replaces the default complete CI gate. Reject
# misspellings rather than silently running no cases or claiming a full pass.
scope = apps.os.environ.get("SUZAKU_VOCABULARY_QA_SCOPE", "all")
if scope not in {"all", "home", "daily", "packs", "part-1", "part-2"}:
    raise ValueError("SUZAKU_VOCABULARY_QA_SCOPE must be all, home, daily, packs, part-1 or part-2")
ALL_CASES = CASES + HOME_CASES + ERRANDS_CASES + DAILY_CASES + DAILY_NEEDS_CASES
PART_CASES = {"part-1": ALL_CASES[::2], "part-2": ALL_CASES[1::2]}
# Compare whole case tuples, including repeated entries if ever intentional:
# both partitions together must preserve the exact full gate, without omissions.
assert Counter(PART_CASES["part-1"] + PART_CASES["part-2"]) == Counter(ALL_CASES)
assert all(cases and {case[0] for case in cases} == {"en", "zh-Hans"}
           for cases in PART_CASES.values()), "each vocabulary part must cover both languages"
CASES = {"home": HOME_CASES, "daily": DAILY_CASES + DAILY_NEEDS_CASES,
         "packs": PACK_CASES, **PART_CASES}.get(scope, ALL_CASES)


def prepare_packs():
    """Exercise the selected installed CLI, only in the harness's private data directory."""
    assert apps.os.environ["XDG_DATA_HOME"] == str(apps.root / "data")
    assert not apps.os.environ.get("SUZAKU_LEXICON_DIR")
    tool = apps.bins / "suzaku_tool"
    expected = [f"org.suzaku.{language}.{topic}" for topic in ("study", "cooking", "travel", "work")
                for language in ("en", "zh-hans")]
    for identifier in expected:
        destination = apps.root / (identifier + ".json")
        for arguments in [("export", identifier, str(destination)), ("install", str(destination))]:
            apps.subprocess.run([str(tool), "pack", *arguments], check=True,
                                capture_output=True, text=True, timeout=10)
    result = apps.subprocess.run([str(tool), "pack", "list"], check=True,
                                 capture_output=True, text=True, timeout=10)
    report = json.loads(result.stdout)
    assert not report["next_startup"]["errors"]
    assert [p["manifest"]["id"] for p in report["packages"] if p["enabled"]] == expected
    assert len(report["next_startup"]["loaded"]) == len(expected)
    print("PASS: installed CLI exported and installed all 8 bilingual packs into private QA data", flush=True)


def check_vocabulary(bus, x):
    document = apps.root / "suzaku-vocabulary-qa.txt"
    document.write_text("")
    apps.spawn(["gnome-text-editor", "--standalone", str(document)])
    apps.wait(lambda: x.window(document.name), "vocabulary editor window")
    editor_window = x.window(document.name)
    x.focus(editor_window)
    x.click(200, 100)
    assert bus.set_global_engine("dev.suzaku.linux.ime")
    apps.settle_input(bus, x, editor_window)
    apps.prepare_editor(x, document)
    print(f"READY: offline vocabulary scope={scope} in the real GTK editor using {apps.bins}", flush=True)
    panel = apps.spawn([str(apps.bins / "panel")]) if scope == "packs" else None

    for language, reading, word, sentence in CASES:
        apps.clear_document(x, document)
        revision = apps.watch.latest["revision"]
        assert json.loads(apps.command("L" + language))["ok"]
        apps.wait(lambda: apps.watch.latest["revision"] > revision
                  and apps.watch.latest["language"] == language, "vocabulary language applied")
        x.type(reading)
        apps.wait(lambda: apps.seed_is(reading), "physical vocabulary spelling")
        if panel is not None:
            # The companion previews the selected Chinese conversion, while
            # the host's editable seed above must still be the exact pinyin.
            preview = word if language == "zh-Hans" else reading
            apps.wait(lambda: apps.companion_frame().get("runtime_font")
                      and apps.companion_frame().get("draft") == preview,
                      "installed companion renders the physical pack draft", timeout=30)
            assert panel.poll() is None and x.focused() == editor_window
        candidates = apps.watch.latest["candidates"][:6]
        for text, kind, label in [(word, "word", "ᵂ"), (sentence, "sentence", "ˢ")]:
            assert any(c["text"] == text and c["kind"] == kind and c["source"] == "local"
                       and label in c["ibus_label"] for c in candidates), (reading, kind, candidates)
            apps.choose_number(x, text)
            apps.save_document(x, document, "")
            x.key(IBus.KEY_BackSpace)
            apps.wait(lambda: apps.seed_is(reading), "exact spelling restored after adoption")

        apps.choose_number(x, word)
        x.key(IBus.KEY_space)
        apps.wait(lambda: apps.seed_is(word + " "), "Space keeps the adopted draft editable")
        if language == "zh-Hans" or sentence.startswith(word + " "):
            remainder = sentence[len(word):]
            if language == "en":
                remainder = remainder.lstrip()
            expected_sentence = word + " " + remainder
            assert any(c["text"] == expected_sentence and c["kind"] == "sentence"
                       and c["source"] == "local" for c in apps.watch.latest["candidates"][:6]), (
                           reading, expected_sentence, apps.watch.latest)
        suffix = "today" if language == "en" else "de"
        x.type(suffix)
        apps.wait(lambda: apps.seed_is(word + " " + suffix), "Space continues adopted vocabulary")
        continued = word + (" today" if language == "en" else " 的")
        if language == "zh-Hans":
            apps.choose_number(x, continued)
        apps.save_document(x, document, "")
        apps.commit(x)
        apps.save_document(x, document, continued + "\n")

        # Exercise actual sentence submission too, not only its presence/undo.
        apps.clear_document(x, document)
        x.type(reading)
        apps.wait(lambda: apps.seed_is(reading), "physical spelling before sentence commit")
        apps.choose_number(x, sentence)
        apps.save_document(x, document, "")
        apps.commit(x)
        apps.save_document(x, document, sentence + "\n")
        print(f"PASS: {language} {reading!r}: word/sentence labels, adoption, undo, Space, exact saved commits",
              flush=True)

    print(f"PASS: all {len(CASES)} offline vocabulary workflows (scope={scope})", flush=True)

    # N58: keep a finite authored sentence through multiple physical word adoptions.
    # No companion-injected draft, actual editor contents observed on every save.
    progress_cases = [
        ("fa yin", "发音", "ke yi", "可以", "zai", "再示范一下吗？"),
        ("shu ru fa", "输入法", "zhi chi", "支持", "", "多种语言。"),
    ] if scope != "part-2" else []
    for reading, word, next_reading, next_word, last_reading, remainder in progress_cases:
        apps.clear_document(x, document)
        revision = apps.watch.latest["revision"]
        assert json.loads(apps.command("Lzh-Hans"))["ok"]
        apps.wait(lambda: apps.watch.latest["revision"] > revision, "sentence progress language")
        x.type(reading)
        apps.wait(lambda: apps.seed_is(reading), "sentence progress physical reading")
        apps.choose_number(x, word)
        x.type(" " + next_reading)
        raw = word + " " + next_reading
        apps.wait(lambda: apps.seed_is(raw), "sentence progress second reading")
        converted = word + " " + next_word
        assert any(c["text"] == converted + remainder and c["kind"] == "sentence"
                   for c in apps.watch.latest["candidates"][:6]), apps.watch.latest
        apps.choose_number(x, converted)
        apps.save_document(x, document, "")
        x.key(IBus.KEY_BackSpace)
        apps.wait(lambda: apps.seed_is(raw), "second-word adoption undo keeps its original spelling")
        apps.choose_number(x, converted)
        x.key(IBus.KEY_space)
        draft = converted + " "
        apps.wait(lambda: apps.seed_is(draft), "sentence progress literal Space")
        sentence = draft + remainder
        assert any(c["text"] == sentence and c["kind"] == "sentence"
                   for c in apps.watch.latest["candidates"][:6]), apps.watch.latest
        if last_reading:
            x.type(last_reading)
            draft += last_reading
            apps.wait(lambda: apps.seed_is(draft), "sentence progress homophone reading")
        apps.choose_number(x, sentence)
        apps.save_document(x, document, "")
        x.key(IBus.KEY_BackSpace)
        apps.wait(lambda: apps.seed_is(draft), "sentence progress sentence undo")
        apps.choose_number(x, sentence)
        apps.commit(x)
        apps.save_document(x, document, sentence + "\n")
        print(f"PASS: sentence progress {reading!r}: consecutive words, Space, homophones, undo and exact saved sentence",
              flush=True)
    print(f"PASS: all {len(CASES) + len(progress_cases)} vocabulary and sentence-progress workflows (scope={scope})", flush=True)


if __name__ == "__main__":
    try:
        if scope == "packs":
            prepare_packs()
        bus, x = apps.start()
        check_vocabulary(bus, x)
    except Exception:
        apps.diagnose()
        raise
    finally:
        apps.close()
