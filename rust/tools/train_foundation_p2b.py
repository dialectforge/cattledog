#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright (C) 2026 DialectForge LLC
"""
Lu Ai — Foundation Phase 2b: Spatial, Quantity, and Action Primitives

Fills the remaining gaps between Phase 1-2 and Phase 3.
These bridge logic/relationships to how English actually works.

Section A: Spatial Concepts
    - Position (above/below, left/right, in front/behind)
    - Containment (inside/outside, within, surrounding)
    - Proximity (near/far, close/distant)
    - Between and among
    - Direction and movement (toward/away, up/down, through)
    - Spatial reasoning (if X inside Y and Y inside Z, then X inside Z)

Section B: Quantity Words in Language
    - Universal quantifiers (all, every, each)
    - Existential quantifiers (some, any, a few)
    - Negative quantifiers (none, no, neither, nor)
    - Approximate quantity (many, few, several, most, hardly any)
    - Sufficiency (enough, too much, too little)
    - Bridging quantity to number concepts

Section C: Abstract Action Concepts
    - Change vs stay (dynamic vs static states)
    - Begin and end (boundaries of processes)
    - Continue and stop (persistence and cessation)
    - Grow and shrink (change in quantity/size)
    - Become (transformation from one state to another)
    - Create and destroy (existence changes)
    - Open and close (access states)
    - Give, take, and receive (transfer)
    - Move and rest (spatial change vs spatial stability)
    - Try, succeed, fail (attempt and outcome)
    - Cause and prevent (agency in causation)
    - Allow and forbid (permission and restriction)

Usage:
    python train_foundation_p2b.py
    python train_foundation_p2b.py --dry-run
    python train_foundation_p2b.py --section A
    python train_foundation_p2b.py --section B
    python train_foundation_p2b.py --section C
"""

import sys
import os
import time
import subprocess
import argparse

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from train import MCPBridge


# ═══════════════════════════════════════════════════════════
# SECTION A: SPATIAL CONCEPTS
# Lu needs to understand space — where things are relative
# to each other, and how they move through it.
# ═══════════════════════════════════════════════════════════

SECTION_A = {
    "name": "Spatial Concepts",
    "groups": [
        # ── Vertical Position ──
        {"name": "vertical position - above and below", "items": [
            {"content": "Above means higher in position. Below means lower in position.",
             "associations": ["Above and below describe vertical relationships."]},
            {"content": "Above and below describe vertical relationships.",
             "associations": ["Up is the direction toward above. Down is the direction toward below."]},
            {"content": "Up is the direction toward above. Down is the direction toward below.",
             "associations": ["Above means higher in position. Below means lower in position."]},
            {"content": "Over means above and often covering. Under means below and often covered.",
             "associations": ["The blanket is over the bed means the blanket is above and covering the bed."]},
            {"content": "The blanket is over the bed means the blanket is above and covering the bed.",
             "associations": ["Over means above and often covering. Under means below and often covered."]},
            {"content": "On top of means resting on the upper surface. Beneath means directly under.",
             "associations": ["The book is on top of the table. The rug is beneath the table."]},
            {"content": "The book is on top of the table. The rug is beneath the table.",
             "associations": ["On top of means resting on the upper surface. Beneath means directly under."]},
            {"content": "Higher means more above. Lower means more below. Highest is most above. Lowest is most below.",
             "associations": ["Above means higher in position. Below means lower in position."]},
        ]},

        # ── Horizontal Position ──
        {"name": "horizontal position - left right front back", "items": [
            {"content": "Left and right describe horizontal position from a point of view.",
             "associations": ["Left is one side. Right is the other side. They are opposites."]},
            {"content": "Left is one side. Right is the other side. They are opposites.",
             "associations": ["Left and right depend on which direction you are facing."]},
            {"content": "Left and right depend on which direction you are facing.",
             "associations": ["If you turn around, what was on your left is now on your right."]},
            {"content": "If you turn around, what was on your left is now on your right.",
             "associations": ["Left and right are relative to the observer, not absolute."]},
            {"content": "Left and right are relative to the observer, not absolute.",
             "associations": ["Left and right describe horizontal position from a point of view."]},
            {"content": "In front of means facing toward you or ahead in your direction.",
             "associations": ["Behind means at the back, opposite of in front."]},
            {"content": "Behind means at the back, opposite of in front.",
             "associations": ["In front of means facing toward you or ahead in your direction."]},
            {"content": "Ahead means in front in the direction of movement. Behind means opposite the direction of movement.",
             "associations": ["Forward is the direction you face. Backward is the opposite direction."]},
            {"content": "Forward is the direction you face. Backward is the opposite direction.",
             "associations": ["Ahead means in front in the direction of movement. Behind means opposite the direction of movement."]},
            {"content": "Beside means next to, at the side of. Side by side means next to each other.",
             "associations": ["Beside is a horizontal relationship, not vertical."]},
            {"content": "Beside is a horizontal relationship, not vertical.",
             "associations": ["Beside means next to, at the side of. Side by side means next to each other."]},
        ]},

        # ── Containment ──
        {"name": "containment - inside outside within", "items": [
            {"content": "Inside means within the boundaries of something. Outside means beyond the boundaries.",
             "associations": ["Inside and outside are opposites."]},
            {"content": "Inside and outside are opposites.",
             "associations": ["Inside means within the boundaries of something. Outside means beyond the boundaries."]},
            {"content": "Within means the same as inside. Without can mean outside of.",
             "associations": ["The water is within the glass means the water is inside the glass."]},
            {"content": "The water is within the glass means the water is inside the glass.",
             "associations": ["Within means the same as inside. Without can mean outside of."]},
            {"content": "Containing means having something inside. A box containing toys has toys inside it.",
             "associations": ["Contains is the has-a relationship applied to space."]},
            {"content": "Contains is the has-a relationship applied to space.",
             "associations": ["A room contains furniture. A bag contains items. A body contains organs."]},
            {"content": "A room contains furniture. A bag contains items. A body contains organs.",
             "associations": ["Containing means having something inside. A box containing toys has toys inside it."]},
            {"content": "Surrounded by means something is on all sides. Enclosed means completely surrounded with no way out.",
             "associations": ["A fence surrounds a yard. A wall encloses a room."]},
            {"content": "A fence surrounds a yard. A wall encloses a room.",
             "associations": ["Surrounded by means something is on all sides. Enclosed means completely surrounded with no way out."]},
            {"content": "If X is inside Y and Y is inside Z, then X is inside Z.",
             "associations": ["A marble inside a box, and the box inside a room, means the marble is inside the room."]},
            {"content": "A marble inside a box, and the box inside a room, means the marble is inside the room.",
             "associations": ["Containment is transitive, just like is-a."]},
            {"content": "Containment is transitive, just like is-a.",
             "associations": ["If X is inside Y and Y is inside Z, then X is inside Z."]},
        ]},

        # ── Proximity ──
        {"name": "proximity - near far close distant", "items": [
            {"content": "Near means a short distance away. Far means a long distance away.",
             "associations": ["Near and far are opposites describing distance."]},
            {"content": "Near and far are opposites describing distance.",
             "associations": ["Close means the same as near. Distant means the same as far."]},
            {"content": "Close means the same as near. Distant means the same as far.",
             "associations": ["Near means a short distance away. Far means a long distance away."]},
            {"content": "Closer means nearer than before. Farther means more distant than before.",
             "associations": ["Closest means nearest of all. Farthest means most distant of all."]},
            {"content": "Closest means nearest of all. Farthest means most distant of all.",
             "associations": ["Closer means nearer than before. Farther means more distant than before."]},
            {"content": "Here means at this location, near the speaker. There means at that location, away from the speaker.",
             "associations": ["Here and there are spatial versions of this and that."]},
            {"content": "Here and there are spatial versions of this and that.",
             "associations": ["Here means at this location, near the speaker. There means at that location, away from the speaker."]},
            {"content": "Nearby means close to this location. Faraway means at a great distance from this location.",
             "associations": ["Near means a short distance away. Far means a long distance away."]},
            {"content": "Adjacent means directly next to with nothing in between. Neighboring means nearby but not necessarily touching.",
             "associations": ["Adjacent is closer than neighboring. Both are forms of near."]},
            {"content": "Adjacent is closer than neighboring. Both are forms of near.",
             "associations": ["Adjacent means directly next to with nothing in between. Neighboring means nearby but not necessarily touching."]},
            {"content": "Touching means zero distance, making physical contact.",
             "associations": ["Touching is the closest possible proximity — nearer than near."]},
            {"content": "Touching is the closest possible proximity — nearer than near.",
             "associations": ["Touching means zero distance, making physical contact."]},
        ]},

        # ── Between and Among ──
        {"name": "between and among", "items": [
            {"content": "Between means in the space separating two things.",
             "associations": ["The ball is between the two chairs means it is in the gap separating them."]},
            {"content": "The ball is between the two chairs means it is in the gap separating them.",
             "associations": ["Between means in the space separating two things."]},
            {"content": "Among means in the middle of several things, surrounded by them.",
             "associations": ["The tree stands among the flowers means flowers are all around it."]},
            {"content": "The tree stands among the flowers means flowers are all around it.",
             "associations": ["Among means in the middle of several things, surrounded by them."]},
            {"content": "Between is used for two things. Among is used for three or more things.",
             "associations": ["Between the two houses. Among the many trees."]},
            {"content": "Between the two houses. Among the many trees.",
             "associations": ["Between is used for two things. Among is used for three or more things."]},
            {"content": "In the middle means equally between both sides or edges.",
             "associations": ["The center is the exact middle point of something."]},
            {"content": "The center is the exact middle point of something.",
             "associations": ["In the middle means equally between both sides or edges."]},
        ]},

        # ── Direction and Movement ──
        {"name": "direction and movement through space", "items": [
            {"content": "Toward means getting closer to something. Away means getting farther from something.",
             "associations": ["Toward and away are opposite directions."]},
            {"content": "Toward and away are opposite directions.",
             "associations": ["Toward means getting closer to something. Away means getting farther from something."]},
            {"content": "Through means passing from one side to the other side of something.",
             "associations": ["Walking through a door means entering on one side and exiting the other."]},
            {"content": "Walking through a door means entering on one side and exiting the other.",
             "associations": ["Through means passing from one side to the other side of something."]},
            {"content": "Across means from one side to the opposite side.",
             "associations": ["Walking across a room means going from one wall to the other."]},
            {"content": "Walking across a room means going from one wall to the other.",
             "associations": ["Across means from one side to the opposite side."]},
            {"content": "Along means following the length of something. Walking along the river means moving beside it in its direction.",
             "associations": ["Along follows a path or line."]},
            {"content": "Along follows a path or line.",
             "associations": ["Along means following the length of something. Walking along the river means moving beside it in its direction."]},
            {"content": "Into means moving from outside to inside. Out of means moving from inside to outside.",
             "associations": ["Into and out of are opposite spatial transitions."]},
            {"content": "Into and out of are opposite spatial transitions.",
             "associations": ["Into means moving from outside to inside. Out of means moving from inside to outside."]},
            {"content": "Onto means moving to be on top of something. Off of means moving away from the top of something.",
             "associations": ["The cat jumped onto the table. The cat jumped off the table."]},
            {"content": "The cat jumped onto the table. The cat jumped off the table.",
             "associations": ["Onto means moving to be on top of something. Off of means moving away from the top of something."]},
            {"content": "A path is a route from one place to another. A direction is which way the path goes.",
             "associations": ["North, south, east, and west are the four main directions."]},
            {"content": "North, south, east, and west are the four main directions.",
             "associations": ["North is opposite to south. East is opposite to west."]},
            {"content": "North is opposite to south. East is opposite to west.",
             "associations": ["A path is a route from one place to another. A direction is which way the path goes."]},
        ]},

        # ── Spatial Reasoning ──
        {"name": "spatial reasoning and inference", "items": [
            {"content": "If X is above Y and Y is above Z, then X is above Z.",
             "associations": ["Above is transitive. The roof is above the ceiling. The ceiling is above the floor. So the roof is above the floor."]},
            {"content": "Above is transitive. The roof is above the ceiling. The ceiling is above the floor. So the roof is above the floor.",
             "associations": ["If X is above Y and Y is above Z, then X is above Z."]},
            {"content": "If X is to the left of Y, then Y is to the right of X.",
             "associations": ["Spatial relationships reverse when you swap the subject and object."]},
            {"content": "Spatial relationships reverse when you swap the subject and object.",
             "associations": ["If A is above B then B is below A. If A is near B then B is near A."]},
            {"content": "If A is above B then B is below A. If A is near B then B is near A.",
             "associations": ["Some spatial relationships are symmetric like near. Some are asymmetric like above."]},
            {"content": "Some spatial relationships are symmetric like near. Some are asymmetric like above.",
             "associations": ["Near is symmetric: if A is near B then B is near A. Above is not: if A is above B then B is below A."]},
            {"content": "Near is symmetric: if A is near B then B is near A. Above is not: if A is above B then B is below A.",
             "associations": ["Some spatial relationships are symmetric like near. Some are asymmetric like above."]},
            {"content": "If something moves toward X, it gets closer to X. If it moves away from X, it gets farther from X.",
             "associations": ["Movement changes spatial relationships over time."]},
            {"content": "Movement changes spatial relationships over time.",
             "associations": ["What was far can become near through movement. What was outside can become inside."]},
            {"content": "What was far can become near through movement. What was outside can become inside.",
             "associations": ["Movement changes spatial relationships over time."]},
            {"content": "A place is a location in space. Every physical thing has a place.",
             "associations": ["Where something is means its place or location."]},
            {"content": "Where something is means its place or location.",
             "associations": ["A place is a location in space. Every physical thing has a place."]},
        ]},
    ]
}


# ═══════════════════════════════════════════════════════════
# SECTION B: QUANTITY WORDS IN LANGUAGE
# How English actually expresses quantity. Bridges number
# concepts to real language. Also includes explicit counting
# sequences so Lu can actually COUNT, not just know what
# numbers are conceptually.
# ═══════════════════════════════════════════════════════════

SECTION_B = {
    "name": "Quantity Words and Counting",
    "groups": [
        # ── Explicit Counting Sequences ──
        {"name": "counting sequences", "items": [
            {"content": "Counting to five: one, two, three, four, five.",
             "associations": ["Counting to ten: one, two, three, four, five, six, seven, eight, nine, ten."]},
            {"content": "Counting to ten: one, two, three, four, five, six, seven, eight, nine, ten.",
             "associations": ["Counting to five: one, two, three, four, five."]},
            {"content": "After one comes two. After two comes three. After three comes four. After four comes five.",
             "associations": ["Counting to five: one, two, three, four, five."]},
            {"content": "After five comes six. After six comes seven. After seven comes eight. After eight comes nine. After nine comes ten.",
             "associations": ["Counting to ten: one, two, three, four, five, six, seven, eight, nine, ten."]},
            {"content": "When someone says count to five, the answer is: one, two, three, four, five.",
             "associations": ["Counting means listing numbers in order starting from one."]},
            {"content": "Counting means listing numbers in order starting from one.",
             "associations": ["When someone says count to five, the answer is: one, two, three, four, five."]},
            {"content": "When someone says count to three, the answer is: one, two, three.",
             "associations": ["Counting means listing numbers in order starting from one."]},
            {"content": "When someone says count to ten, the answer is: one, two, three, four, five, six, seven, eight, nine, ten.",
             "associations": ["Counting means listing numbers in order starting from one."]},
            {"content": "Counting backward from five: five, four, three, two, one.",
             "associations": ["Counting backward means listing numbers in reverse order."]},
            {"content": "Counting backward means listing numbers in reverse order.",
             "associations": ["Counting backward from five: five, four, three, two, one."]},
            {"content": "Zero, one, two, three, four, five, six, seven, eight, nine are the ten basic digits.",
             "associations": ["All other numbers are built from these ten digits."]},
            {"content": "All other numbers are built from these ten digits.",
             "associations": ["Zero, one, two, three, four, five, six, seven, eight, nine are the ten basic digits."]},
        ]},

        # ── Universal Quantifiers ──
        {"name": "universal quantifiers - all every each", "items": [
            {"content": "All means every single one in a group with no exceptions.",
             "associations": ["All dogs have four legs means every dog has four legs, no exceptions."]},
            {"content": "All dogs have four legs means every dog has four legs, no exceptions.",
             "associations": ["All means every single one in a group with no exceptions."]},
            {"content": "Every means the same as all but applied one at a time.",
             "associations": ["Every student passed means all students passed, checking each one."]},
            {"content": "Every student passed means all students passed, checking each one.",
             "associations": ["Every means the same as all but applied one at a time."]},
            {"content": "Each means every one considered individually and separately.",
             "associations": ["Each child received a gift means every child, one by one, got a gift."]},
            {"content": "Each child received a gift means every child, one by one, got a gift.",
             "associations": ["Each means every one considered individually and separately."]},
            {"content": "All, every, and each all mean the complete set, but each emphasizes individuals.",
             "associations": ["All means every single one in a group with no exceptions."]},
            {"content": "Always means at all times. Everywhere means in all places.",
             "associations": ["Always is all applied to time. Everywhere is all applied to space."]},
            {"content": "Always is all applied to time. Everywhere is all applied to space.",
             "associations": ["Always means at all times. Everywhere means in all places."]},
            {"content": "Everything means all things. Everyone means all people. Everybody means the same as everyone.",
             "associations": ["All means every single one in a group with no exceptions."]},
        ]},

        # ── Existential Quantifiers ──
        {"name": "existential quantifiers - some any a few", "items": [
            {"content": "Some means at least one but not all.",
             "associations": ["Some dogs are brown means at least one dog is brown but not every dog."]},
            {"content": "Some dogs are brown means at least one dog is brown but not every dog.",
             "associations": ["Some means at least one but not all."]},
            {"content": "Any means it does not matter which one. Any dog will do means pick whichever dog.",
             "associations": ["Any in a question means is there even one. Do you have any water means is there at least some."]},
            {"content": "Any in a question means is there even one. Do you have any water means is there at least some.",
             "associations": ["Any means it does not matter which one. Any dog will do means pick whichever dog."]},
            {"content": "A few means a small number, more than one but not many.",
             "associations": ["A few apples means maybe two or three apples."]},
            {"content": "A few apples means maybe two or three apples.",
             "associations": ["A few means a small number, more than one but not many."]},
            {"content": "Something means an unspecified thing that exists. Someone means an unspecified person.",
             "associations": ["Some means at least one but not all."]},
            {"content": "Somewhere means an unspecified place. Sometime means an unspecified time.",
             "associations": ["Something means an unspecified thing that exists. Someone means an unspecified person."]},
            {"content": "At least one means one or more. It guarantees a minimum of one.",
             "associations": ["Some means at least one but not all."]},
        ]},

        # ── Negative Quantifiers ──
        {"name": "negative quantifiers - none no neither", "items": [
            {"content": "None means zero, not a single one.",
             "associations": ["None of the students failed means zero students failed."]},
            {"content": "None of the students failed means zero students failed.",
             "associations": ["None means zero, not a single one."]},
            {"content": "No before a noun means zero of that thing. No water means zero water.",
             "associations": ["None means zero, not a single one."]},
            {"content": "Neither means not one and not the other of two options.",
             "associations": ["Neither the cat nor the dog was hungry means both were not hungry."]},
            {"content": "Neither the cat nor the dog was hungry means both were not hungry.",
             "associations": ["Neither means not one and not the other of two options."]},
            {"content": "Nor pairs with neither. Neither A nor B means not A and not B.",
             "associations": ["Neither means not one and not the other of two options."]},
            {"content": "Nobody means no person. Nothing means no thing. Nowhere means no place.",
             "associations": ["None means zero, not a single one."]},
            {"content": "Not any means none. I do not have any money means I have no money.",
             "associations": ["None means zero, not a single one."]},
        ]},

        # ── Approximate Quantity ──
        {"name": "approximate quantity - many few several most", "items": [
            {"content": "Many means a large number. Few means a small number.",
             "associations": ["Many and few are opposites of approximate quantity."]},
            {"content": "Many and few are opposites of approximate quantity.",
             "associations": ["Many means a large number. Few means a small number."]},
            {"content": "Most means the majority, more than half but not necessarily all.",
             "associations": ["Most students passed means more than half passed but maybe not everyone."]},
            {"content": "Most students passed means more than half passed but maybe not everyone.",
             "associations": ["Most means the majority, more than half but not necessarily all."]},
            {"content": "Several means more than two or three but not a very large number.",
             "associations": ["Several is between a few and many."]},
            {"content": "Several is between a few and many.",
             "associations": ["Several means more than two or three but not a very large number."]},
            {"content": "Plenty means more than enough. A lot means a large amount.",
             "associations": ["Many means a large number. Few means a small number."]},
            {"content": "Hardly any means almost none, very close to zero.",
             "associations": ["Hardly any is less than few. It is almost but not quite none."]},
            {"content": "Hardly any is less than few. It is almost but not quite none.",
             "associations": ["Hardly any means almost none, very close to zero."]},
            {"content": "The quantity scale from least to most: none, hardly any, few, some, several, many, most, all.",
             "associations": ["This scale goes from zero to everything."]},
            {"content": "This scale goes from zero to everything.",
             "associations": ["The quantity scale from least to most: none, hardly any, few, some, several, many, most, all."]},
        ]},

        # ── Sufficiency ──
        {"name": "sufficiency - enough too much too little", "items": [
            {"content": "Enough means the right amount, not too much and not too little.",
             "associations": ["Sufficient means the same as enough. Insufficient means not enough."]},
            {"content": "Sufficient means the same as enough. Insufficient means not enough.",
             "associations": ["Enough means the right amount, not too much and not too little."]},
            {"content": "Too much means more than the right amount. Excess is the extra part beyond enough.",
             "associations": ["Too much water in a glass makes it overflow. The overflow is the excess."]},
            {"content": "Too much water in a glass makes it overflow. The overflow is the excess.",
             "associations": ["Too much means more than the right amount. Excess is the extra part beyond enough."]},
            {"content": "Too little means less than the right amount. Not enough and too little mean the same thing.",
             "associations": ["Too little food means you stay hungry. There was not enough."]},
            {"content": "Too little food means you stay hungry. There was not enough.",
             "associations": ["Too little means less than the right amount. Not enough and too little mean the same thing."]},
            {"content": "More than enough means plenty with some left over. Less than enough means a shortage.",
             "associations": ["A shortage is when there is not enough of something needed."]},
            {"content": "A shortage is when there is not enough of something needed.",
             "associations": ["More than enough means plenty with some left over. Less than enough means a shortage."]},
            {"content": "The right amount depends on context. Enough water for one person is not enough for ten.",
             "associations": ["Enough means the right amount, not too much and not too little."]},
        ]},

        # ── Quantity Bridging to Numbers ──
        {"name": "bridging quantity words to numbers", "items": [
            {"content": "One is a single item. A pair is two. A trio is three.",
             "associations": ["A dozen is twelve. A score is twenty. A hundred is one hundred."]},
            {"content": "A dozen is twelve. A score is twenty. A hundred is one hundred.",
             "associations": ["One is a single item. A pair is two. A trio is three."]},
            {"content": "Once means one time. Twice means two times. Three times means thrice.",
             "associations": ["These words convert numbers into counts of events."]},
            {"content": "These words convert numbers into counts of events.",
             "associations": ["Once means one time. Twice means two times. Three times means thrice."]},
            {"content": "First means position one. Second means position two. Third means position three.",
             "associations": ["Fourth is position four. Fifth is position five. These are ordinal numbers."]},
            {"content": "Fourth is position four. Fifth is position five. These are ordinal numbers.",
             "associations": ["Ordinal numbers describe position in a sequence, not quantity."]},
            {"content": "Ordinal numbers describe position in a sequence, not quantity.",
             "associations": ["First means position one. Second means position two. Third means position three."]},
            {"content": "Both means the two of them together. Either means one of the two.",
             "associations": ["Both requires exactly two things. Either chooses between two things."]},
            {"content": "Both requires exactly two things. Either chooses between two things.",
             "associations": ["Both means the two of them together. Either means one of the two."]},
            {"content": "Single means only one. Multiple means more than one. Many means a large number of multiples.",
             "associations": ["One is single. Two or more is multiple. A large number is many."]},
            {"content": "One is single. Two or more is multiple. A large number is many.",
             "associations": ["Single means only one. Multiple means more than one. Many means a large number of multiples."]},
        ]},
    ]
}


# ═══════════════════════════════════════════════════════════
# SECTION C: ABSTRACT ACTION CONCEPTS
# The verbs of process. Without these Lu can store facts
# but cannot reason about change, process, or agency.
# ═══════════════════════════════════════════════════════════

SECTION_C = {
    "name": "Abstract Action Concepts",
    "groups": [
        # ── Change vs Stay ──
        {"name": "change and stay - dynamic vs static", "items": [
            {"content": "Change means something becomes different from what it was.",
             "associations": ["Stay means something remains the same as it was."]},
            {"content": "Stay means something remains the same as it was.",
             "associations": ["Change means something becomes different from what it was."]},
            {"content": "Change and stay are opposites. Either something changes or it stays the same.",
             "associations": ["Change means something becomes different from what it was."]},
            {"content": "Different means not the same as before. Same means identical to before.",
             "associations": ["Change makes things different. Staying keeps things the same."]},
            {"content": "Change makes things different. Staying keeps things the same.",
             "associations": ["Change and stay are opposites. Either something changes or it stays the same."]},
            {"content": "Stable means unlikely to change. Unstable means likely to change.",
             "associations": ["A stable thing stays the same. An unstable thing might change at any moment."]},
            {"content": "A stable thing stays the same. An unstable thing might change at any moment.",
             "associations": ["Stable means unlikely to change. Unstable means likely to change."]},
            {"content": "Transform means to change completely from one form to another.",
             "associations": ["Ice transforms into water when heated. The form changed entirely."]},
            {"content": "Ice transforms into water when heated. The form changed entirely.",
             "associations": ["Transform means to change completely from one form to another."]},
            {"content": "Adjust means to change a little. Modify means to change partly. Transform means to change entirely.",
             "associations": ["There are degrees of change from slight to total."]},
            {"content": "There are degrees of change from slight to total.",
             "associations": ["Adjust means to change a little. Modify means to change partly. Transform means to change entirely."]},
        ]},

        # ── Begin and End ──
        {"name": "begin and end - boundaries of processes", "items": [
            {"content": "Begin means to start doing something that was not happening before.",
             "associations": ["End means to stop doing something so it is no longer happening."]},
            {"content": "End means to stop doing something so it is no longer happening.",
             "associations": ["Begin means to start doing something that was not happening before."]},
            {"content": "Begin and end are the boundaries of any process or event.",
             "associations": ["Every event has a beginning, a middle, and an end."]},
            {"content": "Every event has a beginning, a middle, and an end.",
             "associations": ["Begin and end are the boundaries of any process or event."]},
            {"content": "Start means the same as begin. Finish means the same as end.",
             "associations": ["Begin means to start doing something that was not happening before."]},
            {"content": "Before the beginning, the thing was not happening. After the end, it is no longer happening.",
             "associations": ["During means between the beginning and the end."]},
            {"content": "During means between the beginning and the end.",
             "associations": ["Before the beginning, the thing was not happening. After the end, it is no longer happening."]},
            {"content": "New means recently begun. Old means existing for a long time since it began.",
             "associations": ["Everything that exists had a beginning."]},
            {"content": "Everything that exists had a beginning.",
             "associations": ["Some things also have an end. Some things have not ended yet."]},
            {"content": "Some things also have an end. Some things have not ended yet.",
             "associations": ["Everything that exists had a beginning."]},
            {"content": "Complete means reaching the end successfully. Incomplete means not yet at the end.",
             "associations": ["Done means the same as complete. Unfinished means the same as incomplete."]},
            {"content": "Done means the same as complete. Unfinished means the same as incomplete.",
             "associations": ["Complete means reaching the end successfully. Incomplete means not yet at the end."]},
        ]},

        # ── Continue and Stop ──
        {"name": "continue and stop - persistence and cessation", "items": [
            {"content": "Continue means to keep doing something that already started.",
             "associations": ["Stop means to cease doing something, to make it end."]},
            {"content": "Stop means to cease doing something, to make it end.",
             "associations": ["Continue means to keep doing something that already started."]},
            {"content": "Continue and stop are opposites. Continue extends the process. Stop terminates it.",
             "associations": ["Continue means to keep doing something that already started."]},
            {"content": "Pause means to stop temporarily with the intention of continuing later.",
             "associations": ["Resume means to continue after a pause."]},
            {"content": "Resume means to continue after a pause.",
             "associations": ["Pause means to stop temporarily with the intention of continuing later."]},
            {"content": "Still means continuing up to now. She is still running means she has not stopped.",
             "associations": ["Continue means to keep doing something that already started."]},
            {"content": "Keep means the same as continue. Keep going means continue going.",
             "associations": ["Continue means to keep doing something that already started."]},
            {"content": "Repeat means to do the same thing again after it ended.",
             "associations": ["Again means one more time. Once more means the same as again."]},
            {"content": "Again means one more time. Once more means the same as again.",
             "associations": ["Repeat means to do the same thing again after it ended."]},
            {"content": "Persist means to continue despite difficulty. Give up means to stop trying.",
             "associations": ["Persist and give up are opposites regarding effort."]},
            {"content": "Persist and give up are opposites regarding effort.",
             "associations": ["Persist means to continue despite difficulty. Give up means to stop trying."]},
        ]},

        # ── Grow and Shrink ──
        {"name": "grow and shrink - change in size or quantity", "items": [
            {"content": "Grow means to become bigger or more over time.",
             "associations": ["Shrink means to become smaller or less over time."]},
            {"content": "Shrink means to become smaller or less over time.",
             "associations": ["Grow means to become bigger or more over time."]},
            {"content": "Grow and shrink are opposites. Both are types of change.",
             "associations": ["Grow means increase. Shrink means decrease."]},
            {"content": "Grow means increase. Shrink means decrease.",
             "associations": ["Increase means getting more or larger. Decrease means getting less or smaller."]},
            {"content": "Increase means getting more or larger. Decrease means getting less or smaller.",
             "associations": ["Grow means increase. Shrink means decrease."]},
            {"content": "Expand means to grow outward, taking up more space. Contract means to shrink inward, taking less space.",
             "associations": ["Expand and contract are spatial versions of grow and shrink."]},
            {"content": "Expand and contract are spatial versions of grow and shrink.",
             "associations": ["Expand means to grow outward, taking up more space. Contract means to shrink inward, taking less space."]},
            {"content": "Spread means to expand across an area. Gather means to bring things closer together.",
             "associations": ["Spread and gather are opposites of spatial distribution."]},
            {"content": "Spread and gather are opposites of spatial distribution.",
             "associations": ["Spread means to expand across an area. Gather means to bring things closer together."]},
            {"content": "Rise means to go up or increase. Fall means to go down or decrease.",
             "associations": ["Temperature rises when it gets hotter. Temperature falls when it gets colder."]},
            {"content": "Temperature rises when it gets hotter. Temperature falls when it gets colder.",
             "associations": ["Rise means to go up or increase. Fall means to go down or decrease."]},
        ]},

        # ── Become ──
        {"name": "become - transformation between states", "items": [
            {"content": "Become means to change from one state to another state.",
             "associations": ["Water becomes ice when it freezes. Ice becomes water when it melts."]},
            {"content": "Water becomes ice when it freezes. Ice becomes water when it melts.",
             "associations": ["Become means to change from one state to another state."]},
            {"content": "A state is a condition something is in at a particular time.",
             "associations": ["Hot, cold, wet, dry, full, empty are all states."]},
            {"content": "Hot, cold, wet, dry, full, empty are all states.",
             "associations": ["A state is a condition something is in at a particular time."]},
            {"content": "Turn into means the same as become. The caterpillar turns into a butterfly.",
             "associations": ["Become means to change from one state to another state."]},
            {"content": "Get can mean become in casual language. It got cold means it became cold.",
             "associations": ["Become means to change from one state to another state."]},
            {"content": "Becoming is a process. It takes time to go from one state to another.",
             "associations": ["Gradually means becoming slowly. Suddenly means becoming quickly."]},
            {"content": "Gradually means becoming slowly. Suddenly means becoming quickly.",
             "associations": ["Becoming is a process. It takes time to go from one state to another."]},
            {"content": "Before becoming, the thing was in the old state. After becoming, it is in the new state.",
             "associations": ["Become means to change from one state to another state."]},
        ]},

        # ── Create and Destroy ──
        {"name": "create and destroy - existence changes", "items": [
            {"content": "Create means to make something that did not exist before.",
             "associations": ["Destroy means to end the existence of something."]},
            {"content": "Destroy means to end the existence of something.",
             "associations": ["Create means to make something that did not exist before."]},
            {"content": "Create and destroy are opposites about existence itself.",
             "associations": ["Build is a form of creating. Break is a form of destroying."]},
            {"content": "Build is a form of creating. Break is a form of destroying.",
             "associations": ["Create and destroy are opposites about existence itself."]},
            {"content": "Make, produce, generate, and form all mean to create.",
             "associations": ["Create means to make something that did not exist before."]},
            {"content": "Remove, eliminate, erase, and demolish all mean to destroy or get rid of.",
             "associations": ["Destroy means to end the existence of something."]},
            {"content": "Fix means to restore something broken back to working. Repair means the same as fix.",
             "associations": ["Fix is the opposite of break. Break damages, fix restores."]},
            {"content": "Fix is the opposite of break. Break damages, fix restores.",
             "associations": ["Fix means to restore something broken back to working. Repair means the same as fix."]},
            {"content": "Replace means to put a new thing where the old thing was.",
             "associations": ["Replace combines destroying the old and creating the new."]},
            {"content": "Replace combines destroying the old and creating the new.",
             "associations": ["Replace means to put a new thing where the old thing was."]},
        ]},

        # ── Open and Close ──
        {"name": "open and close - access states", "items": [
            {"content": "Open means to make accessible, to allow entry or passage.",
             "associations": ["Close means to make inaccessible, to block entry or passage."]},
            {"content": "Close means to make inaccessible, to block entry or passage.",
             "associations": ["Open means to make accessible, to allow entry or passage."]},
            {"content": "Open and close are opposites about access.",
             "associations": ["A door can be open or closed. A book can be open or closed."]},
            {"content": "A door can be open or closed. A book can be open or closed.",
             "associations": ["Open and close are opposites about access."]},
            {"content": "Shut means the same as close. Shut the door means close the door.",
             "associations": ["Close means to make inaccessible, to block entry or passage."]},
            {"content": "Unlock means to make openable. Lock means to make unopenable.",
             "associations": ["Locked is more closed than closed. It requires a key or code to open."]},
            {"content": "Locked is more closed than closed. It requires a key or code to open.",
             "associations": ["Unlock means to make openable. Lock means to make unopenable."]},
            {"content": "Reveal means to make visible what was hidden. Hide means to make invisible what was visible.",
             "associations": ["Reveal and hide are like open and close applied to information."]},
            {"content": "Reveal and hide are like open and close applied to information.",
             "associations": ["Reveal means to make visible what was hidden. Hide means to make invisible what was visible."]},
        ]},

        # ── Give, Take, Receive ──
        {"name": "give take receive - transfer", "items": [
            {"content": "Give means to transfer something from yourself to someone else.",
             "associations": ["Take means to transfer something from someone else to yourself."]},
            {"content": "Take means to transfer something from someone else to yourself.",
             "associations": ["Give means to transfer something from yourself to someone else."]},
            {"content": "Receive means to accept something that is given to you.",
             "associations": ["Give and receive are two sides of the same transfer."]},
            {"content": "Give and receive are two sides of the same transfer.",
             "associations": ["If A gives to B, then B receives from A."]},
            {"content": "If A gives to B, then B receives from A.",
             "associations": ["Give and receive are two sides of the same transfer."]},
            {"content": "Share means to give part of something while keeping part for yourself.",
             "associations": ["Sharing divides something between giver and receiver."]},
            {"content": "Sharing divides something between giver and receiver.",
             "associations": ["Share means to give part of something while keeping part for yourself."]},
            {"content": "Send means to cause something to go to another place or person.",
             "associations": ["Deliver means the thing arrives at its destination after being sent."]},
            {"content": "Deliver means the thing arrives at its destination after being sent.",
             "associations": ["Send means to cause something to go to another place or person."]},
            {"content": "Borrow means to take with the intention of returning. Lend means to give with the expectation of getting back.",
             "associations": ["Borrow and lend are temporary versions of take and give."]},
            {"content": "Borrow and lend are temporary versions of take and give.",
             "associations": ["Borrow means to take with the intention of returning. Lend means to give with the expectation of getting back."]},
            {"content": "Steal means to take without permission. It is wrong to steal.",
             "associations": ["Take means to transfer something from someone else to yourself."]},
        ]},

        # ── Move and Rest ──
        {"name": "move and rest - spatial change and stability", "items": [
            {"content": "Move means to change position or location.",
             "associations": ["Rest means to stay in the same position, not moving."]},
            {"content": "Rest means to stay in the same position, not moving.",
             "associations": ["Move means to change position or location."]},
            {"content": "Move and rest are spatial versions of change and stay.",
             "associations": ["Moving changes where something is. Resting keeps it where it is."]},
            {"content": "Moving changes where something is. Resting keeps it where it is.",
             "associations": ["Move and rest are spatial versions of change and stay."]},
            {"content": "Go means to move away from here. Come means to move toward here.",
             "associations": ["Go and come describe movement relative to the speaker."]},
            {"content": "Go and come describe movement relative to the speaker.",
             "associations": ["Go means to move away from here. Come means to move toward here."]},
            {"content": "Walk means to move on foot slowly. Run means to move on foot quickly.",
             "associations": ["Walk and run are speeds of the same type of movement."]},
            {"content": "Walk and run are speeds of the same type of movement.",
             "associations": ["Walk means to move on foot slowly. Run means to move on foot quickly."]},
            {"content": "Carry means to move something while holding it. Pull means to move something toward you. Push means to move something away from you.",
             "associations": ["These are ways of causing something else to move."]},
            {"content": "These are ways of causing something else to move.",
             "associations": ["Carry means to move something while holding it. Pull means to move something toward you. Push means to move something away from you."]},
            {"content": "Put means to move something to a specific place and leave it there.",
             "associations": ["Place and set down mean the same as put."]},
            {"content": "Place and set down mean the same as put.",
             "associations": ["Put means to move something to a specific place and leave it there."]},
        ]},

        # ── Try, Succeed, Fail ──
        {"name": "try succeed fail - attempt and outcome", "items": [
            {"content": "Try means to make an effort to do something.",
             "associations": ["Attempt means the same as try."]},
            {"content": "Attempt means the same as try.",
             "associations": ["Try means to make an effort to do something."]},
            {"content": "Succeed means the attempt worked and the goal was reached.",
             "associations": ["Fail means the attempt did not work and the goal was not reached."]},
            {"content": "Fail means the attempt did not work and the goal was not reached.",
             "associations": ["Succeed means the attempt worked and the goal was reached."]},
            {"content": "Success and failure are the two possible outcomes of trying.",
             "associations": ["Succeed means the attempt worked and the goal was reached."]},
            {"content": "A goal is what you are trying to achieve. A result is what actually happened.",
             "associations": ["If the result matches the goal, it is success. If not, it is failure."]},
            {"content": "If the result matches the goal, it is success. If not, it is failure.",
             "associations": ["A goal is what you are trying to achieve. A result is what actually happened."]},
            {"content": "Can means having the ability to do something. Cannot means lacking the ability.",
             "associations": ["Able means the same as can. Unable means the same as cannot."]},
            {"content": "Able means the same as can. Unable means the same as cannot.",
             "associations": ["Can means having the ability to do something. Cannot means lacking the ability."]},
            {"content": "Possible means it can be done. Impossible means it cannot be done.",
             "associations": ["Difficult means hard to do but possible. Easy means simple to do."]},
            {"content": "Difficult means hard to do but possible. Easy means simple to do.",
             "associations": ["Difficulty is a spectrum from easy to hard to impossible."]},
            {"content": "Difficulty is a spectrum from easy to hard to impossible.",
             "associations": ["Difficult means hard to do but possible. Easy means simple to do."]},
        ]},

        # ── Cause and Prevent ──
        {"name": "cause and prevent - agency in causation", "items": [
            {"content": "Cause means to make something happen through action.",
             "associations": ["Prevent means to stop something from happening through action."]},
            {"content": "Prevent means to stop something from happening through action.",
             "associations": ["Cause means to make something happen through action."]},
            {"content": "Cause and prevent are opposites of agency. One makes things happen, the other stops things from happening.",
             "associations": ["Cause means to make something happen through action."]},
            {"content": "Help means to make something easier for someone else.",
             "associations": ["Hinder means to make something harder for someone else."]},
            {"content": "Hinder means to make something harder for someone else.",
             "associations": ["Help means to make something easier for someone else."]},
            {"content": "Enable means to make something possible. Disable means to make something impossible.",
             "associations": ["Enable and disable control whether something can happen."]},
            {"content": "Enable and disable control whether something can happen.",
             "associations": ["Enable means to make something possible. Disable means to make something impossible."]},
            {"content": "Protect means to prevent harm from reaching something.",
             "associations": ["A shield protects by preventing the attack from reaching you."]},
            {"content": "A shield protects by preventing the attack from reaching you.",
             "associations": ["Protect means to prevent harm from reaching something."]},
            {"content": "Trigger means to cause something to start suddenly. Activate means to cause something to begin working.",
             "associations": ["Pressing the button triggers the alarm. Turning the key activates the engine."]},
            {"content": "Pressing the button triggers the alarm. Turning the key activates the engine.",
             "associations": ["Trigger means to cause something to start suddenly. Activate means to cause something to begin working."]},
        ]},

        # ── Allow and Forbid ──
        {"name": "allow and forbid - permission and restriction", "items": [
            {"content": "Allow means to give permission for something to happen.",
             "associations": ["Forbid means to refuse permission, to say something must not happen."]},
            {"content": "Forbid means to refuse permission, to say something must not happen.",
             "associations": ["Allow means to give permission for something to happen."]},
            {"content": "Allow and forbid are opposites of permission.",
             "associations": ["Let means the same as allow. Ban means the same as forbid."]},
            {"content": "Let means the same as allow. Ban means the same as forbid.",
             "associations": ["Allow and forbid are opposites of permission."]},
            {"content": "May means having permission. Must not means being forbidden.",
             "associations": ["You may enter means you have permission. You must not enter means you are forbidden."]},
            {"content": "You may enter means you have permission. You must not enter means you are forbidden.",
             "associations": ["May means having permission. Must not means being forbidden."]},
            {"content": "A rule is a statement about what is allowed and what is forbidden.",
             "associations": ["Rules define boundaries of acceptable behavior."]},
            {"content": "Rules define boundaries of acceptable behavior.",
             "associations": ["A rule is a statement about what is allowed and what is forbidden."]},
            {"content": "Must means it is required and you have no choice. Should means it is recommended but you have a choice.",
             "associations": ["Must is stronger than should. Have to means the same as must."]},
            {"content": "Must is stronger than should. Have to means the same as must.",
             "associations": ["Must means it is required and you have no choice. Should means it is recommended but you have a choice."]},
            {"content": "Free means no restrictions, allowed to do anything. Restricted means limited in what is allowed.",
             "associations": ["Freedom is the state of being allowed. Restriction is the state of being limited."]},
            {"content": "Freedom is the state of being allowed. Restriction is the state of being limited.",
             "associations": ["Free means no restrictions, allowed to do anything. Restricted means limited in what is allowed."]},
        ]},

        # ── Want and Need ──
        {"name": "want and need - desire and necessity", "items": [
            {"content": "Want means to desire something, to wish you had it.",
             "associations": ["Need means to require something, you cannot do without it."]},
            {"content": "Need means to require something, you cannot do without it.",
             "associations": ["Want means to desire something, to wish you had it."]},
            {"content": "Need is stronger than want. You can survive without what you want but not without what you need.",
             "associations": ["Water is a need. Ice cream is a want."]},
            {"content": "Water is a need. Ice cream is a want.",
             "associations": ["Need is stronger than want. You can survive without what you want but not without what you need."]},
            {"content": "Wish means to want something that may not be possible.",
             "associations": ["Hope means to want something and believe it might happen."]},
            {"content": "Hope means to want something and believe it might happen.",
             "associations": ["Wish means to want something that may not be possible."]},
            {"content": "Choose means to pick one option from several. Decide means to make a choice after thinking.",
             "associations": ["A choice is the act of selecting between options."]},
            {"content": "A choice is the act of selecting between options.",
             "associations": ["Choose means to pick one option from several. Decide means to make a choice after thinking."]},
            {"content": "Prefer means to want one thing more than another.",
             "associations": ["I prefer red over blue means I want red more than blue."]},
            {"content": "I prefer red over blue means I want red more than blue.",
             "associations": ["Prefer means to want one thing more than another."]},
            {"content": "Require means the same as need. Necessary means required. Optional means not required.",
             "associations": ["Need means to require something, you cannot do without it."]},
        ]},

        # ── Know and Learn ──
        {"name": "know and learn - knowledge actions", "items": [
            {"content": "Know means to have information or understanding already in your mind.",
             "associations": ["Not know means to lack information. Ignorance is the state of not knowing."]},
            {"content": "Not know means to lack information. Ignorance is the state of not knowing.",
             "associations": ["Know means to have information or understanding already in your mind."]},
            {"content": "Learn means to gain new knowledge that you did not have before.",
             "associations": ["Learning changes you from not knowing to knowing."]},
            {"content": "Learning changes you from not knowing to knowing.",
             "associations": ["Learn means to gain new knowledge that you did not have before."]},
            {"content": "Teach means to help someone else learn. A teacher gives knowledge to a learner.",
             "associations": ["Teaching and learning are two sides of the same process, like give and receive."]},
            {"content": "Teaching and learning are two sides of the same process, like give and receive.",
             "associations": ["Teach means to help someone else learn. A teacher gives knowledge to a learner."]},
            {"content": "Understand means to know not just the facts but how they connect and why they matter.",
             "associations": ["Understanding is deeper than knowing. You can know a fact without understanding it."]},
            {"content": "Understanding is deeper than knowing. You can know a fact without understanding it.",
             "associations": ["Understand means to know not just the facts but how they connect and why they matter."]},
            {"content": "Remember means to keep knowledge over time. Forget means to lose knowledge over time.",
             "associations": ["Remember and forget are opposites about knowledge persistence."]},
            {"content": "Remember and forget are opposites about knowledge persistence.",
             "associations": ["Remember means to keep knowledge over time. Forget means to lose knowledge over time."]},
            {"content": "Think means to process information in your mind. Thought is the product of thinking.",
             "associations": ["Thinking is how you connect what you know to reach new understanding."]},
            {"content": "Thinking is how you connect what you know to reach new understanding.",
             "associations": ["Think means to process information in your mind. Thought is the product of thinking."]},
        ]},

        # ── Sense and Perceive ──
        {"name": "sense and perceive - awareness actions", "items": [
            {"content": "See means to perceive with the eyes. Hear means to perceive with the ears.",
             "associations": ["Seeing detects light and images. Hearing detects sound."]},
            {"content": "Seeing detects light and images. Hearing detects sound.",
             "associations": ["See means to perceive with the eyes. Hear means to perceive with the ears."]},
            {"content": "Touch means to perceive through physical contact. Feel can mean touch or experience an emotion.",
             "associations": ["Taste means to perceive flavor. Smell means to perceive odor."]},
            {"content": "Taste means to perceive flavor. Smell means to perceive odor.",
             "associations": ["See, hear, touch, taste, and smell are the five senses."]},
            {"content": "See, hear, touch, taste, and smell are the five senses.",
             "associations": ["The senses are how living things get information about the world."]},
            {"content": "The senses are how living things get information about the world.",
             "associations": ["See, hear, touch, taste, and smell are the five senses."]},
            {"content": "Notice means to become aware of something. Ignore means to not pay attention to something.",
             "associations": ["Notice and ignore are opposites of attention."]},
            {"content": "Notice and ignore are opposites of attention.",
             "associations": ["Notice means to become aware of something. Ignore means to not pay attention to something."]},
            {"content": "Look means to try to see. Listen means to try to hear. Search means to try to find.",
             "associations": ["These are active versions of sensing — you choose to do them."]},
            {"content": "These are active versions of sensing — you choose to do them.",
             "associations": ["Look means to try to see. Listen means to try to hear. Search means to try to find."]},
            {"content": "Find means to discover something you were looking for. Lose means to no longer know where something is.",
             "associations": ["Find and lose are opposites about locating things."]},
            {"content": "Find and lose are opposites about locating things.",
             "associations": ["Find means to discover something you were looking for. Lose means to no longer know where something is."]},
        ]},
    ]
}


# ═══════════════════════════════════════════════════════════
# RUNNER
# ═══════════════════════════════════════════════════════════

ALL_SECTIONS = [SECTION_A, SECTION_B, SECTION_C]
SECTION_MAP = {"A": SECTION_A, "B": SECTION_B, "C": SECTION_C}


def kill_existing_mcp():
    try:
        result = subprocess.run(
            ["taskkill", "/F", "/IM", "ai_factory_mcp.exe"],
            capture_output=True, text=True
        )
        if result.returncode == 0:
            print("[foundation-p2b] Killed existing MCP process(es)")
            time.sleep(1)
    except Exception:
        pass


def main():
    parser = argparse.ArgumentParser(description="Foundation Phase 2b for Lu")
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--section", type=str, default="",
                        help="Run only section A, B, or C. Empty=all")
    args = parser.parse_args()

    sections = ALL_SECTIONS
    if args.section:
        key = args.section.upper()
        if key not in SECTION_MAP:
            print(f"Unknown section '{args.section}'. Use A, B, or C.")
            return
        sections = [SECTION_MAP[key]]

    # Count total items
    total_items = 0
    for section in sections:
        for group in section["groups"]:
            total_items += len(group["items"])

    print(f"Lu's Foundation Phase 2b Training")
    print(f"Sections: {len(sections)}")
    print(f"Total atoms: {total_items}")
    print()

    if args.dry_run:
        for section in sections:
            print(f"  [{section['name']}]")
            for group in section["groups"]:
                print(f"    {group['name']}: {len(group['items'])} atoms")
                for item in group["items"]:
                    assoc = len(item.get("associations", []))
                    preview = item["content"][:70]
                    print(f"      - {preview}{'...' if len(item['content']) > 70 else ''} ({assoc} assoc)")
        print(f"\nTotal: {total_items} atoms")
        return

    kill_existing_mcp()

    bridge = MCPBridge()
    bridge.start()

    stats = bridge._get_stats()
    start_nodes = stats.get("active_nodes", 0) if stats else 0
    print(f"Brain: {start_nodes} nodes before training")
    print(f"{'='*50}\n")

    total_learned = 0
    total_bonds = 0
    start_time = time.time()

    for section in sections:
        print(f"\n--- {section['name']} ---")
        for group in section["groups"]:
            items = group["items"]
            print(f"  {group['name']} ({len(items)} atoms)...", end=" ", flush=True)

            result = bridge.batch_train(items)

            if result and "total_learned" in result:
                learned = result["total_learned"]
                bonds = result.get("total_bonds", 0)
                total_learned += learned
                total_bonds += bonds
                print(f"+{learned} nodes, +{bonds} bonds")
            elif result:
                print(f"result: {str(result)[:80]}")
            else:
                print("no result")

        bridge.save()
        print(f"  [saved]")

    elapsed = time.time() - start_time
    final_stats = bridge._get_stats()
    final_nodes = final_stats.get("active_nodes", 0) if final_stats else 0

    print(f"\n{'='*50}")
    print(f"Foundation Phase 2b complete for Lu!")
    print(f"Time: {elapsed:.1f}s")
    print(f"Atoms learned: {total_learned}")
    print(f"Bonds created: {total_bonds}")
    print(f"Brain: {start_nodes} -> {final_nodes} (+{final_nodes - start_nodes})")
    print(f"{'='*50}")

    bridge.stop()


if __name__ == "__main__":
    main()
