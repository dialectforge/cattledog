#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright (C) 2026 DialectForge LLC
"""
AI Factory Overnight Training Script v4

Feeds structured knowledge into the brain via the MCP server's
brain_batch_train tool. Sends JSON-RPC over stdio to the MCP binary.

Usage:
    python overnight_train_v4.py
    python overnight_train_v4.py --dry-run    (preview without training)
    python overnight_train_v4.py --batch-size 10

The script spawns the MCP server as a subprocess, sends batch_train
commands, and tracks progress. Each topic is a batch of related facts
with associations between them.
"""

import subprocess
import json
import sys
import time
import os
import signal
import threading

# ─── Config ──────────────────────────────────────────
MCP_BINARY = r"<AI_FACTORY_ROOT>\target\release\lu_ai_mcp.exe"
BRAIN_PATH = r"<AI_FACTORY_ROOT>\brain"
BATCH_SIZE = 5  # items per batch_train call
DRY_RUN = "--dry-run" in sys.argv

# ─── Knowledge Topics ────────────────────────────────
# Each topic is a list of items with content and associations.
# Associations link related facts within the same topic.
TOPICS = [
    # ── Mathematics ──────────────────────────────
    {
        "name": "Arithmetic Basics",
        "items": [
            {"content": "Addition is a mathematical operation that combines two or more numbers into a sum.", "associations": ["Subtraction is the inverse of addition."]},
            {"content": "Subtraction is the inverse of addition.", "associations": ["Addition is a mathematical operation that combines two or more numbers into a sum."]},
            {"content": "Multiplication is repeated addition of the same number.", "associations": ["Division is the inverse of multiplication."]},
            {"content": "Division is the inverse of multiplication.", "associations": ["Multiplication is repeated addition of the same number."]},
            {"content": "The order of operations in math is parentheses, exponents, multiplication, division, addition, subtraction."},
        ]
    },
    {
        "name": "Geometry",
        "items": [
            {"content": "A triangle is a polygon with three sides and three angles.", "associations": ["The sum of angles in a triangle is always 180 degrees."]},
            {"content": "The sum of angles in a triangle is always 180 degrees."},
            {"content": "A circle is a shape where all points are the same distance from the center.", "associations": ["Pi is the ratio of a circle's circumference to its diameter, approximately 3.14159."]},
            {"content": "Pi is the ratio of a circle's circumference to its diameter, approximately 3.14159."},
            {"content": "The Pythagorean theorem states that in a right triangle, the square of the hypotenuse equals the sum of the squares of the other two sides."},
            {"content": "A square is a rectangle where all four sides are equal length."},
        ]
    },
    {
        "name": "Algebra",
        "items": [
            {"content": "Algebra uses letters and symbols to represent numbers and quantities in equations."},
            {"content": "A variable is a symbol, usually a letter, that represents an unknown value in an equation."},
            {"content": "An equation states that two expressions are equal, connected by an equals sign."},
            {"content": "The quadratic formula solves equations of the form ax squared plus bx plus c equals zero."},
            {"content": "A function maps each input to exactly one output."},
        ]
    },

    # ── Science ──────────────────────────────────
    {
        "name": "Chemistry Basics",
        "items": [
            {"content": "An atom is the smallest unit of an element that retains the chemical properties of that element.", "associations": ["Atoms consist of protons, neutrons, and electrons."]},
            {"content": "Atoms consist of protons, neutrons, and electrons.", "associations": ["Protons have a positive charge, electrons have a negative charge, and neutrons are neutral."]},
            {"content": "Protons have a positive charge, electrons have a negative charge, and neutrons are neutral."},
            {"content": "A molecule is two or more atoms bonded together.", "associations": ["Water is a molecule made of two hydrogen atoms bonded to one oxygen atom essential for all life."]},
            {"content": "The periodic table organizes all known elements by their atomic number and chemical properties."},
            {"content": "A chemical reaction occurs when atoms rearrange to form new substances."},
        ]
    },
    {
        "name": "Biology Cells",
        "items": [
            {"content": "A cell is the basic unit of life that makes up all living organisms.", "associations": ["There are two main types of cells: prokaryotic cells without a nucleus and eukaryotic cells with a nucleus."]},
            {"content": "There are two main types of cells: prokaryotic cells without a nucleus and eukaryotic cells with a nucleus."},
            {"content": "DNA is a molecule that carries the genetic instructions for the development and functioning of all living things.", "associations": ["DNA is shaped like a double helix, a twisted ladder structure."]},
            {"content": "DNA is shaped like a double helix, a twisted ladder structure."},
            {"content": "Mitosis is the process by which a cell divides into two identical daughter cells."},
            {"content": "Proteins are large molecules that perform most of the work in cells and are essential for the structure and function of the body."},
        ]
    },
    {
        "name": "Human Body",
        "items": [
            {"content": "The human body has 206 bones in the adult skeleton.", "associations": ["Bones provide structure, protect organs, and allow movement."]},
            {"content": "Bones provide structure, protect organs, and allow movement."},
            {"content": "The heart pumps blood through the circulatory system, delivering oxygen and nutrients to every cell.", "associations": ["Blood carries oxygen from the lungs to the body and carbon dioxide back to the lungs."]},
            {"content": "Blood carries oxygen from the lungs to the body and carbon dioxide back to the lungs."},
            {"content": "The brain is the control center of the nervous system, processing information from the senses and directing the body's responses."},
            {"content": "Muscles contract and relax to produce movement, and the human body has over 600 muscles."},
        ]
    },
    {
        "name": "Ecology",
        "items": [
            {"content": "An ecosystem is a community of living organisms interacting with their physical environment.", "associations": ["Food chains show how energy flows from one organism to another in an ecosystem."]},
            {"content": "Food chains show how energy flows from one organism to another in an ecosystem.", "associations": ["Producers like plants make their own food, while consumers eat other organisms."]},
            {"content": "Producers like plants make their own food, while consumers eat other organisms."},
            {"content": "Decomposers break down dead organisms and recycle nutrients back into the ecosystem."},
            {"content": "Biodiversity refers to the variety of life forms in an ecosystem, and higher biodiversity usually means a healthier ecosystem."},
        ]
    },

    # ── Technology ────────────────────────────────
    {
        "name": "Computers",
        "items": [
            {"content": "A computer is an electronic device that processes data according to a set of instructions called a program.", "associations": ["The CPU is the central processing unit, the brain of the computer that executes instructions."]},
            {"content": "The CPU is the central processing unit, the brain of the computer that executes instructions."},
            {"content": "RAM is random access memory, temporary storage that the computer uses while running programs.", "associations": ["A hard drive or SSD stores data permanently, even when the computer is turned off."]},
            {"content": "A hard drive or SSD stores data permanently, even when the computer is turned off."},
            {"content": "An operating system is software that manages computer hardware and provides services for other programs."},
            {"content": "Binary code uses only zeros and ones to represent all data and instructions in a computer."},
        ]
    },
    {
        "name": "Internet",
        "items": [
            {"content": "The internet is a global network of interconnected computers that communicate using standardized protocols.", "associations": ["TCP/IP is the set of protocols that governs how data is transmitted across the internet."]},
            {"content": "TCP/IP is the set of protocols that governs how data is transmitted across the internet."},
            {"content": "A web browser is software that retrieves and displays web pages from the internet."},
            {"content": "HTML is the standard markup language used to create web pages.", "associations": ["CSS is a language used to style and layout web pages."]},
            {"content": "CSS is a language used to style and layout web pages."},
            {"content": "Encryption scrambles data so that only authorized parties can read it, protecting privacy online."},
        ]
    },
    {
        "name": "Artificial Intelligence",
        "items": [
            {"content": "Artificial intelligence is the simulation of human intelligence by machines, especially computer systems."},
            {"content": "Machine learning is a subset of AI where systems learn from data rather than being explicitly programmed.", "associations": ["A neural network is a computing system inspired by the biological neural networks in the brain."]},
            {"content": "A neural network is a computing system inspired by the biological neural networks in the brain."},
            {"content": "Natural language processing enables computers to understand, interpret, and generate human language."},
            {"content": "Computer vision enables machines to interpret and understand visual information from the world."},
        ]
    },

    # ── History ───────────────────────────────────
    {
        "name": "Ancient Civilizations",
        "items": [
            {"content": "Ancient Egypt developed along the Nile River and built the pyramids as tombs for pharaohs.", "associations": ["The Great Pyramid of Giza is one of the Seven Wonders of the Ancient World."]},
            {"content": "The Great Pyramid of Giza is one of the Seven Wonders of the Ancient World."},
            {"content": "Ancient Greece is considered the birthplace of democracy, philosophy, and the Olympic Games.", "associations": ["Athens was a center of learning and philosophy in Ancient Greece."]},
            {"content": "Athens was a center of learning and philosophy in Ancient Greece."},
            {"content": "The Roman Empire at its peak controlled much of Europe, North Africa, and the Middle East.", "associations": ["Rome built roads, aqueducts, and legal systems that influenced Western civilization for centuries."]},
            {"content": "Rome built roads, aqueducts, and legal systems that influenced Western civilization for centuries."},
        ]
    },
    {
        "name": "Scientific Revolution",
        "items": [
            {"content": "The Scientific Revolution in the 16th and 17th centuries transformed how people understood the natural world.", "associations": ["Galileo Galilei used a telescope to observe the moons of Jupiter, supporting the heliocentric model."]},
            {"content": "Galileo Galilei used a telescope to observe the moons of Jupiter, supporting the heliocentric model."},
            {"content": "Isaac Newton formulated the laws of motion and universal gravitation in his work Principia Mathematica.", "associations": ["Newton's three laws of motion describe how forces affect the movement of objects."]},
            {"content": "Newton's three laws of motion describe how forces affect the movement of objects."},
            {"content": "The scientific method is a systematic approach to inquiry involving observation, hypothesis, experiment, and conclusion."},
        ]
    },

    # ── Geography ─────────────────────────────────
    {
        "name": "Continents and Oceans",
        "items": [
            {"content": "There are seven continents: Africa, Antarctica, Asia, Australia, Europe, North America, and South America."},
            {"content": "Asia is the largest continent by both area and population.", "associations": ["Africa is the second largest continent and has the most countries."]},
            {"content": "Africa is the second largest continent and has the most countries."},
            {"content": "The Pacific Ocean is the largest and deepest ocean on Earth.", "associations": ["The Atlantic Ocean is the second largest ocean and separates the Americas from Europe and Africa."]},
            {"content": "The Atlantic Ocean is the second largest ocean and separates the Americas from Europe and Africa."},
            {"content": "Antarctica is the coldest, driest, and windiest continent, covered almost entirely by ice."},
        ]
    },
    {
        "name": "Weather and Climate",
        "items": [
            {"content": "Weather is the short-term state of the atmosphere including temperature, humidity, wind, and precipitation.", "associations": ["Climate is the average weather pattern in a region over a long period of time."]},
            {"content": "Climate is the average weather pattern in a region over a long period of time."},
            {"content": "Clouds form when water vapor in the air cools and condenses into tiny water droplets or ice crystals.", "associations": ["Rain occurs when water droplets in clouds become too heavy and fall to the ground."]},
            {"content": "Rain occurs when water droplets in clouds become too heavy and fall to the ground."},
            {"content": "Wind is caused by differences in air pressure, with air moving from high pressure areas to low pressure areas."},
            {"content": "A hurricane is a large rotating storm system that forms over warm ocean water with winds exceeding 74 miles per hour."},
        ]
    },

    # ── Music ─────────────────────────────────────
    {
        "name": "Music Fundamentals",
        "items": [
            {"content": "Music is organized sound that uses rhythm, melody, and harmony to express ideas and emotions."},
            {"content": "Rhythm is the pattern of beats and silences in music that gives it a sense of movement.", "associations": ["Tempo is the speed at which music is played, measured in beats per minute."]},
            {"content": "Tempo is the speed at which music is played, measured in beats per minute."},
            {"content": "Melody is a sequence of musical notes that form a recognizable tune.", "associations": ["Harmony is the combination of different musical notes played at the same time to create chords."]},
            {"content": "Harmony is the combination of different musical notes played at the same time to create chords."},
            {"content": "An octave is the interval between one musical pitch and another with double its frequency."},
        ]
    },

    # ── Philosophy ────────────────────────────────
    {
        "name": "Major Philosophies",
        "items": [
            {"content": "Philosophy is the study of fundamental questions about existence, knowledge, values, reason, and language."},
            {"content": "Socrates is considered the father of Western philosophy and taught by asking probing questions.", "associations": ["Plato was a student of Socrates and wrote dialogues exploring justice, beauty, and equality."]},
            {"content": "Plato was a student of Socrates and wrote dialogues exploring justice, beauty, and equality.", "associations": ["Aristotle was a student of Plato who made foundational contributions to logic, science, and ethics."]},
            {"content": "Aristotle was a student of Plato who made foundational contributions to logic, science, and ethics."},
            {"content": "Ethics is the branch of philosophy concerned with what is morally right and wrong."},
        ]
    },

    # ── Space ─────────────────────────────────────
    {
        "name": "Solar System",
        "items": [
            {"content": "The solar system consists of the Sun and everything that orbits it, including eight planets.", "associations": ["Mercury is the closest planet to the Sun and the smallest planet in the solar system."]},
            {"content": "Mercury is the closest planet to the Sun and the smallest planet in the solar system."},
            {"content": "Venus is the hottest planet in the solar system due to a thick atmosphere of carbon dioxide.", "associations": ["Mars is called the Red Planet because of iron oxide on its surface."]},
            {"content": "Mars is called the Red Planet because of iron oxide on its surface."},
            {"content": "Jupiter is the largest planet in the solar system, a gas giant with a Great Red Spot storm larger than Earth.", "associations": ["Saturn is famous for its prominent ring system made of ice and rock particles."]},
            {"content": "Saturn is famous for its prominent ring system made of ice and rock particles."},
            {"content": "Neptune is the farthest planet from the Sun and has the strongest winds in the solar system."},
        ]
    },
    {
        "name": "Stars and Galaxies",
        "items": [
            {"content": "A star is a massive ball of hot gas that produces energy through nuclear fusion.", "associations": ["The Sun is a medium-sized star that provides light and heat to Earth."]},
            {"content": "The Sun is a medium-sized star that provides light and heat to Earth."},
            {"content": "A galaxy is a massive system of stars, gas, dust, and dark matter bound together by gravity.", "associations": ["The Milky Way is the galaxy that contains our solar system."]},
            {"content": "The Milky Way is the galaxy that contains our solar system."},
            {"content": "A light-year is the distance light travels in one year, about 5.88 trillion miles."},
            {"content": "A black hole is a region of space where gravity is so strong that nothing, not even light, can escape."},
        ]
    },

    # ── Economics ──────────────────────────────────
    {
        "name": "Economics Basics",
        "items": [
            {"content": "Economics is the study of how societies allocate scarce resources to satisfy unlimited wants.", "associations": ["Supply and demand is the fundamental model that determines prices in a market economy."]},
            {"content": "Supply and demand is the fundamental model that determines prices in a market economy."},
            {"content": "Inflation is the rate at which the general level of prices for goods and services is rising.", "associations": ["Deflation is the opposite of inflation, where prices decrease over time."]},
            {"content": "Deflation is the opposite of inflation, where prices decrease over time."},
            {"content": "GDP or Gross Domestic Product is the total value of goods and services produced in a country in a given period."},
        ]
    },

    # ── Literature ─────────────────────────────────
    {
        "name": "Literary Concepts",
        "items": [
            {"content": "A metaphor is a figure of speech that describes something by calling it something else to suggest a comparison.", "associations": ["A simile is a figure of speech that compares two things using the words like or as."]},
            {"content": "A simile is a figure of speech that compares two things using the words like or as."},
            {"content": "Irony is when the intended meaning is opposite to the literal meaning of words or events."},
            {"content": "A protagonist is the main character of a story, while an antagonist is the character who opposes them."},
            {"content": "Theme is the central message or underlying meaning of a literary work."},
            {"content": "Fiction is literature created from imagination, not based on fact, while nonfiction is based on real events and facts."},
        ]
    },
]


# ─── MCP Client ──────────────────────────────────────
def kill_existing_mcp():
    """Kill any running lu_ai_mcp.exe processes before starting."""
    try:
        result = subprocess.run(
            ["taskkill", "/F", "/IM", "lu_ai_mcp.exe"],
            capture_output=True, text=True
        )
        if result.returncode == 0:
            print("Killed existing MCP process(es)")
            time.sleep(1)
    except Exception:
        pass


class MCPClient:
    TIMEOUT = 120  # seconds per request

    def __init__(self, binary_path, brain_path):
        self.proc = subprocess.Popen(
            [binary_path, brain_path],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=1,
        )
        self.request_id = 0
        # Initialize MCP protocol
        self._send("initialize", {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {"name": "overnight_train", "version": "4.0"}
        })

    def _send(self, method, params=None, timeout=None):
        if timeout is None:
            timeout = self.TIMEOUT
        self.request_id += 1
        msg = {
            "jsonrpc": "2.0",
            "id": self.request_id,
            "method": method,
        }
        if params is not None:
            msg["params"] = params
        line = json.dumps(msg)
        self.proc.stdin.write(line + "\n")
        self.proc.stdin.flush()

        # Read response with timeout
        response_line = [None]
        def read_line():
            response_line[0] = self.proc.stdout.readline()

        reader = threading.Thread(target=read_line, daemon=True)
        reader.start()
        reader.join(timeout=timeout)

        if reader.is_alive():
            print(f"\n  TIMEOUT after {timeout}s — restarting MCP...", flush=True)
            self._restart()
            return {"error": "timeout"}

        if not response_line[0]:
            return None
        return json.loads(response_line[0])

    def _restart(self):
        """Kill and restart the MCP process."""
        try:
            self.proc.kill()
            self.proc.wait(timeout=5)
        except Exception:
            pass
        self.proc = subprocess.Popen(
            [self.proc.args[0], self.proc.args[1]],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=1,
        )
        self.request_id = 0
        self._send("initialize", {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {"name": "overnight_train", "version": "4.0"}
        }, timeout=30)

    def batch_train(self, items):
        """Call brain_batch_train with a list of items."""
        return self._send("tools/call", {
            "name": "brain_batch_train",
            "arguments": {"items": items}
        })

    def stats(self):
        """Call brain_stats."""
        return self._send("tools/call", {
            "name": "brain_stats",
            "arguments": {}
        })

    def save(self):
        """Call brain_save."""
        return self._send("tools/call", {
            "name": "brain_save",
            "arguments": {}
        })

    def close(self):
        self.proc.stdin.close()
        self.proc.wait(timeout=10)


# ─── Main ────────────────────────────────────────────
def main():
    total_items = sum(len(t["items"]) for t in TOPICS)
    print(f"AI Factory Overnight Training v4")
    print(f"Topics: {len(TOPICS)}")
    print(f"Total knowledge items: {total_items}")
    print(f"Batch size: {BATCH_SIZE}")
    print(f"Dry run: {DRY_RUN}")
    print()

    if not DRY_RUN:
        kill_existing_mcp()

    if DRY_RUN:
        for topic in TOPICS:
            print(f"  [{topic['name']}] {len(topic['items'])} items")
            for item in topic["items"]:
                assoc_count = len(item.get("associations", []))
                print(f"    - {item['content'][:80]}... ({assoc_count} assoc)")
        print(f"\nTotal: {total_items} items across {len(TOPICS)} topics")
        return

    # Start MCP server
    print(f"Starting MCP server: {MCP_BINARY}")
    print(f"Brain path: {BRAIN_PATH}")
    client = MCPClient(MCP_BINARY, BRAIN_PATH)

    # Get initial stats
    stats_resp = client.stats()
    if stats_resp:
        print(f"Initial brain stats: {stats_resp.get('result', {})}")
    print()

    total_learned = 0
    total_bonds = 0
    start_time = time.time()

    for topic_idx, topic in enumerate(TOPICS):
        topic_name = topic["name"]
        items = topic["items"]
        print(f"[{topic_idx+1}/{len(TOPICS)}] {topic_name} ({len(items)} items)...", end=" ", flush=True)

        # Send in batches
        for batch_start in range(0, len(items), BATCH_SIZE):
            batch = items[batch_start:batch_start + BATCH_SIZE]

            # Retry up to 2 times on timeout
            for attempt in range(3):
                resp = client.batch_train(batch)
                if resp and resp.get("error") == "timeout":
                    print(f"  Retry {attempt+1}/2...", end=" ", flush=True)
                    continue
                break

            if resp and "result" in resp:
                # Parse the result content
                result_content = resp["result"]
                if isinstance(result_content, dict) and "content" in result_content:
                    for content_item in result_content["content"]:
                        if content_item.get("type") == "text":
                            try:
                                data = json.loads(content_item["text"])
                                total_learned += data.get("total_learned", 0)
                                total_bonds += data.get("total_bonds", 0)
                            except (json.JSONDecodeError, KeyError):
                                pass
            elif resp and "error" in resp:
                print(f"\n  FAILED: {resp['error']}")

        print("done")

    elapsed = time.time() - start_time

    # Save
    client.save()

    # Final stats
    stats_resp = client.stats()
    print(f"\n{'='*50}")
    print(f"Training complete in {elapsed:.1f}s")
    print(f"Items learned: {total_learned}")
    print(f"Bonds created: {total_bonds}")
    if stats_resp:
        print(f"Final brain stats: {stats_resp.get('result', {})}")

    client.close()


if __name__ == "__main__":
    main()
