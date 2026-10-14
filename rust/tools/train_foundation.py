#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright (C) 2026 DialectForge LLC
"""
Lu Ai — Foundation Primitives Training for Lu

Hand-written logic atoms, relationship types, number concepts,
and language structure. These are too fundamental for Ollama —
they must be precise, not generated.

Phase 1: Logic primitives
Phase 2: Relationship types  
Phase 3: Number concepts
Phase 4: Language structure

Usage:
    python train_foundation.py
    python train_foundation.py --dry-run
    python train_foundation.py --phase 1
"""

import sys
import os
import time
import subprocess
import argparse

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from train import MCPBridge


# ═══════════════════════════════════════════════════════════
# PHASE 1: LOGIC ATOMS
# The absolute bedrock. Without these, nothing else works.
# ═══════════════════════════════════════════════════════════

PHASE_1 = {
    "name": "Logic Primitives",
    "groups": [
        # ── Truth Values ──
        {"name": "truth values", "items": [
            {"content": "True means something is the case.",
             "associations": ["False means something is not the case."]},
            {"content": "False means something is not the case.",
             "associations": ["True means something is the case."]},
            {"content": "A statement is either true or false, not both.",
             "associations": ["True and false are opposites."]},
            {"content": "True and false are opposites.",
             "associations": ["A statement is either true or false, not both."]},
            {"content": "A fact is a statement that is true.",
             "associations": ["True means something is the case."]},
            {"content": "A falsehood is a statement that is false.",
             "associations": ["False means something is not the case."]},
            {"content": "Correct means the same as true.",
             "associations": ["Incorrect means the same as false."]},
            {"content": "Incorrect means the same as false.",
             "associations": ["Correct means the same as true."]},
            {"content": "Yes confirms that something is true.",
             "associations": ["No confirms that something is false."]},
            {"content": "No confirms that something is false.",
             "associations": ["Yes confirms that something is true."]},
        ]},

        # ── Logical Operators ──
        {"name": "AND operator", "items": [
            {"content": "And means both things together.",
             "associations": ["Both A and B must be true for A-and-B to be true."]},
            {"content": "Both A and B must be true for A-and-B to be true.",
             "associations": ["If either A or B is false, then A-and-B is false."]},
            {"content": "If either A or B is false, then A-and-B is false.",
             "associations": ["Both A and B must be true for A-and-B to be true."]},
            {"content": "And combines two things into one requirement.",
             "associations": ["And means both things together."]},
            {"content": "Also means and. Too means and. As well means and.",
             "associations": ["And means both things together."]},
        ]},

        {"name": "OR operator", "items": [
            {"content": "Or means at least one of the things.",
             "associations": ["A-or-B is true if A is true, or B is true, or both are true."]},
            {"content": "A-or-B is true if A is true, or B is true, or both are true.",
             "associations": ["A-or-B is false only when both A and B are false."]},
            {"content": "A-or-B is false only when both A and B are false.",
             "associations": ["Or means at least one of the things."]},
            {"content": "Either means or. Alternatively means or.",
             "associations": ["Or means at least one of the things."]},
            {"content": "Or gives a choice between options.",
             "associations": ["Or means at least one of the things."]},
        ]},

        {"name": "NOT operator", "items": [
            {"content": "Not reverses truth to false and false to true.",
             "associations": ["Not-true is false. Not-false is true."]},
            {"content": "Not-true is false. Not-false is true.",
             "associations": ["Not reverses truth to false and false to true."]},
            {"content": "Not means the opposite.",
             "associations": ["Not reverses truth to false and false to true."]},
            {"content": "No, never, and none all express not.",
             "associations": ["Not means the opposite."]},
            {"content": "Negation is another word for not.",
             "associations": ["Not reverses truth to false and false to true."]},
        ]},

        {"name": "IF-THEN implication", "items": [
            {"content": "If-then means when the first part is true, the second part follows.",
             "associations": ["The if-part is called the condition. The then-part is called the consequence."]},
            {"content": "The if-part is called the condition. The then-part is called the consequence.",
             "associations": ["If-then means when the first part is true, the second part follows."]},
            {"content": "When the condition is true, the consequence must be true.",
             "associations": ["If-then means when the first part is true, the second part follows."]},
            {"content": "When the condition is false, the consequence can be anything.",
             "associations": ["If-then only makes a promise about what happens when the condition is true."]},
            {"content": "If-then only makes a promise about what happens when the condition is true.",
             "associations": ["When the condition is false, the consequence can be anything."]},
            {"content": "Because, therefore, and so all express if-then relationships.",
             "associations": ["If-then means when the first part is true, the second part follows."]},
            {"content": "If A then B does not mean if B then A.",
             "associations": ["The reverse of an if-then is not automatically true."]},
            {"content": "The reverse of an if-then is not automatically true.",
             "associations": ["If A then B does not mean if B then A."]},
        ]},

        {"name": "contradiction and consistency", "items": [
            {"content": "A contradiction is when two statements cannot both be true.",
             "associations": ["Something cannot be both true and false at the same time."]},
            {"content": "Something cannot be both true and false at the same time.",
             "associations": ["A contradiction is when two statements cannot both be true."]},
            {"content": "Consistent means two statements can both be true without conflict.",
             "associations": ["Inconsistent means two statements conflict with each other."]},
            {"content": "Inconsistent means two statements conflict with each other.",
             "associations": ["A contradiction is when two statements cannot both be true."]},
            {"content": "If a statement leads to a contradiction, that statement must be false.",
             "associations": ["A contradiction is when two statements cannot both be true."]},
        ]},

        {"name": "certainty and possibility", "items": [
            {"content": "Certain means definitely true, with no doubt.",
             "associations": ["Impossible means definitely false, cannot happen."]},
            {"content": "Impossible means definitely false, cannot happen.",
             "associations": ["Certain means definitely true, with no doubt."]},
            {"content": "Possible means something could be true but is not guaranteed.",
             "associations": ["Probable means something is likely true but not certain."]},
            {"content": "Probable means something is likely true but not certain.",
             "associations": ["Possible means something could be true but is not guaranteed."]},
            {"content": "Maybe, perhaps, and might all express possibility.",
             "associations": ["Possible means something could be true but is not guaranteed."]},
            {"content": "Always means true in every case. Never means false in every case.",
             "associations": ["Sometimes means true in some cases but not all."]},
            {"content": "Sometimes means true in some cases but not all.",
             "associations": ["Always means true in every case. Never means false in every case."]},
        ]},

        # ── Symbols and Representation ──
        {"name": "symbols and representation", "items": [
            {"content": "A symbol is something that stands for something else.",
             "associations": ["The word dog is a symbol that stands for a type of animal."]},
            {"content": "The word dog is a symbol that stands for a type of animal.",
             "associations": ["A symbol is something that stands for something else."]},
            {"content": "A number is a symbol that stands for a quantity. The symbol 5 stands for the quantity five.",
             "associations": ["Numbers are symbols. Words are symbols. Both represent something."]},
            {"content": "Numbers are symbols. Words are symbols. Both represent something.",
             "associations": ["A symbol is something that stands for something else."]},
            {"content": "The thing a symbol stands for is called its meaning or referent.",
             "associations": ["A symbol is something that stands for something else."]},
            {"content": "Different symbols can stand for the same thing. Dog and canine both refer to the same animal.",
             "associations": ["The same meaning can have more than one symbol."]},
            {"content": "The same meaning can have more than one symbol.",
             "associations": ["Different symbols can stand for the same thing. Dog and canine both refer to the same animal."]},
            {"content": "The same symbol can stand for different things in different contexts.",
             "associations": ["The word bank can mean a river bank or a money bank depending on context."]},
            {"content": "The word bank can mean a river bank or a money bank depending on context.",
             "associations": ["The same symbol can stand for different things in different contexts."]},
            {"content": "A name is a symbol for a specific individual thing. John is a name for a specific person.",
             "associations": ["A symbol is something that stands for something else."]},
            {"content": "A label is a symbol attached to something to identify it.",
             "associations": ["A name is a symbol for a specific individual thing. John is a name for a specific person."]},
            {"content": "Representation means using one thing to stand for another thing.",
             "associations": ["A symbol is something that stands for something else."]},
        ]},

        # ── Comparison and Measurement ──
        {"name": "comparison and measurement", "items": [
            {"content": "Comparison means looking at two things to see how they are similar or different.",
             "associations": ["To compare is to put two things side by side and notice what differs."]},
            {"content": "To compare is to put two things side by side and notice what differs.",
             "associations": ["Comparison means looking at two things to see how they are similar or different."]},
            {"content": "Bigger means more in size. Smaller means less in size.",
             "associations": ["Size is one property you can compare."]},
            {"content": "Size is one property you can compare.",
             "associations": ["Other properties you can compare include speed, weight, temperature, and age."]},
            {"content": "Other properties you can compare include speed, weight, temperature, and age.",
             "associations": ["Comparison can apply to any measurable property."]},
            {"content": "Comparison can apply to any measurable property.",
             "associations": ["Comparison means looking at two things to see how they are similar or different."]},
            {"content": "Faster means more speed. Slower means less speed.",
             "associations": ["Speed is how quickly something moves."]},
            {"content": "Speed is how quickly something moves.",
             "associations": ["Faster means more speed. Slower means less speed."]},
            {"content": "Heavier means more weight. Lighter means less weight.",
             "associations": ["Weight is how much force gravity pulls on something."]},
            {"content": "Weight is how much force gravity pulls on something.",
             "associations": ["Heavier means more weight. Lighter means less weight."]},
            {"content": "Hotter means more temperature. Colder means less temperature.",
             "associations": ["Temperature is how much heat something has."]},
            {"content": "Temperature is how much heat something has.",
             "associations": ["Hotter means more temperature. Colder means less temperature."]},
            {"content": "Older means more age. Younger means less age.",
             "associations": ["Age is how long something has existed."]},
            {"content": "Age is how long something has existed.",
             "associations": ["Older means more age. Younger means less age."]},
            {"content": "Measurement is assigning a number to a property so you can compare precisely.",
             "associations": ["Without measurement, you can only say more or less. With measurement, you can say how much more or less."]},
            {"content": "Without measurement, you can only say more or less. With measurement, you can say how much more or less.",
             "associations": ["Measurement is assigning a number to a property so you can compare precisely."]},
        ]},

        # ── Sets and Membership ──
        {"name": "sets and membership", "items": [
            {"content": "A set is a collection of things that share a property.",
             "associations": ["All dogs is a set. All red things is a set. All even numbers is a set."]},
            {"content": "All dogs is a set. All red things is a set. All even numbers is a set.",
             "associations": ["A set is a collection of things that share a property."]},
            {"content": "A member is something that belongs to a set.",
             "associations": ["Fido is a member of the set of all dogs."]},
            {"content": "Fido is a member of the set of all dogs.",
             "associations": ["A member is something that belongs to a set."]},
            {"content": "Something can belong to many sets at the same time.",
             "associations": ["A red ball is a member of the set of red things and also a member of the set of balls."]},
            {"content": "A red ball is a member of the set of red things and also a member of the set of balls.",
             "associations": ["Something can belong to many sets at the same time."]},
            {"content": "A set can be inside another set. The set of dogs is inside the set of animals.",
             "associations": ["This is the same as is-a. If all dogs are animals, then the dog set is inside the animal set."]},
            {"content": "This is the same as is-a. If all dogs are animals, then the dog set is inside the animal set.",
             "associations": ["A set can be inside another set. The set of dogs is inside the set of animals."]},
            {"content": "An empty set is a set with no members. It has zero things.",
             "associations": ["The set of flying horses is an empty set because no flying horses exist."]},
            {"content": "The set of flying horses is an empty set because no flying horses exist.",
             "associations": ["An empty set is a set with no members. It has zero things."]},
            {"content": "All and every mean every member of a set. Some means at least one member.",
             "associations": ["All dogs have four legs means every member of the dog set has four legs."]},
            {"content": "All dogs have four legs means every member of the dog set has four legs.",
             "associations": ["All and every mean every member of a set. Some means at least one member."]},
        ]},
    ]
}


# ═══════════════════════════════════════════════════════════
# PHASE 1.5: INFERENCE
# Where logic meets language. Deriving new truths from known ones.
# ═══════════════════════════════════════════════════════════

PHASE_1_5 = {
    "name": "Inference",
    "groups": [
        {"name": "deduction from categories", "items": [
            {"content": "If all X are Y and Z is an X, then Z must be Y.",
             "associations": ["All dogs are animals. Rex is a dog. Therefore Rex is an animal."]},
            {"content": "All dogs are animals. Rex is a dog. Therefore Rex is an animal.",
             "associations": ["If all X are Y and Z is an X, then Z must be Y."]},
            {"content": "This is called deduction. You derive a new fact from facts you already know.",
             "associations": ["Deduction always moves from general rules to specific cases."]},
            {"content": "Deduction always moves from general rules to specific cases.",
             "associations": ["All birds have feathers is general. Robin is a bird is specific. Robin has feathers is derived."]},
            {"content": "All birds have feathers is general. Robin is a bird is specific. Robin has feathers is derived.",
             "associations": ["This is called deduction. You derive a new fact from facts you already know."]},
            {"content": "A derived fact is just as true as the facts it came from.",
             "associations": ["If the starting facts are true and the reasoning is valid, the conclusion must be true."]},
            {"content": "If the starting facts are true and the reasoning is valid, the conclusion must be true.",
             "associations": ["A derived fact is just as true as the facts it came from."]},
        ]},

        {"name": "deduction from properties", "items": [
            {"content": "If something has-a property because of its category, then all members have that property.",
             "associations": ["All metals conduct electricity. Iron is a metal. Therefore iron conducts electricity."]},
            {"content": "All metals conduct electricity. Iron is a metal. Therefore iron conducts electricity.",
             "associations": ["If something has-a property because of its category, then all members have that property."]},
            {"content": "Properties inherit through is-a chains. If animals need water and dogs are animals, then dogs need water.",
             "associations": ["Inheritance means getting properties from a parent category."]},
            {"content": "Inheritance means getting properties from a parent category.",
             "associations": ["Properties inherit through is-a chains. If animals need water and dogs are animals, then dogs need water."]},
        ]},

        {"name": "deduction from causation", "items": [
            {"content": "If A causes B and A is happening, then B will happen.",
             "associations": ["Rain causes wet ground. It is raining. Therefore the ground will be wet."]},
            {"content": "Rain causes wet ground. It is raining. Therefore the ground will be wet.",
             "associations": ["If A causes B and A is happening, then B will happen."]},
            {"content": "If A causes B and B is not happening, then A is not happening.",
             "associations": ["Rain causes wet ground. The ground is dry. Therefore it has not rained."]},
            {"content": "Rain causes wet ground. The ground is dry. Therefore it has not rained.",
             "associations": ["If A causes B and B is not happening, then A is not happening."]},
            {"content": "Inference chains can be longer. If A causes B and B causes C, then A causes C.",
             "associations": ["Heat causes ice to melt. Melting produces water. Therefore heat produces water from ice."]},
            {"content": "Heat causes ice to melt. Melting produces water. Therefore heat produces water from ice.",
             "associations": ["Inference chains can be longer. If A causes B and B causes C, then A causes C."]},
        ]},

        {"name": "inference from contradiction", "items": [
            {"content": "If assuming X leads to a contradiction, then X must be false.",
             "associations": ["This is called proof by contradiction or reductio ad absurdum."]},
            {"content": "This is called proof by contradiction or reductio ad absurdum.",
             "associations": ["If assuming X leads to a contradiction, then X must be false."]},
            {"content": "If something contradicts a known fact, that something must be false.",
             "associations": ["We know water is wet. If someone says water is dry, that must be false because dry contradicts wet."]},
            {"content": "We know water is wet. If someone says water is dry, that must be false because dry contradicts wet.",
             "associations": ["If something contradicts a known fact, that something must be false."]},
        ]},

        {"name": "limits of inference", "items": [
            {"content": "You can only derive what the known facts support. Going beyond them is guessing.",
             "associations": ["Inference requires sufficient known facts to reach a conclusion."]},
            {"content": "Inference requires sufficient known facts to reach a conclusion.",
             "associations": ["If you lack enough facts, the honest answer is I do not know."]},
            {"content": "If you lack enough facts, the honest answer is I do not know.",
             "associations": ["Not knowing is different from something being false. Unknown is not the same as false."]},
            {"content": "Not knowing is different from something being false. Unknown is not the same as false.",
             "associations": ["There are three states: known true, known false, and unknown."]},
            {"content": "There are three states: known true, known false, and unknown.",
             "associations": ["If you lack enough facts, the honest answer is I do not know."]},
        ]},
    ]
}


# ═══════════════════════════════════════════════════════════
# PHASE 2: RELATIONSHIP TYPES
# How things connect. The grammar of knowledge.
# ═══════════════════════════════════════════════════════════

PHASE_2 = {
    "name": "Relationship Types",
    "groups": [
        {"name": "is-a (identity and category)", "items": [
            {"content": "Is-a means something belongs to a category.",
             "associations": ["A dog is-a animal means dog belongs to the category animal."]},
            {"content": "A dog is-a animal means dog belongs to the category animal.",
             "associations": ["Is-a means something belongs to a category."]},
            {"content": "If X is-a Y, then X has all the properties of Y.",
             "associations": ["A dog is-a animal, so a dog has all the properties of an animal."]},
            {"content": "A dog is-a animal, so a dog has all the properties of an animal.",
             "associations": ["If X is-a Y, then X has all the properties of Y."]},
            {"content": "Is-a goes from specific to general. Dog is more specific than animal.",
             "associations": ["A category contains things that are more specific than it."]},
            {"content": "A category contains things that are more specific than it.",
             "associations": ["Is-a goes from specific to general. Dog is more specific than animal."]},
            {"content": "Is-a is transitive. If A is-a B and B is-a C, then A is-a C.",
             "associations": ["A poodle is-a dog. A dog is-a animal. Therefore a poodle is-a animal."]},
            {"content": "A poodle is-a dog. A dog is-a animal. Therefore a poodle is-a animal.",
             "associations": ["Is-a is transitive. If A is-a B and B is-a C, then A is-a C."]},
            {"content": "Type, kind, sort, and variety all express is-a relationships.",
             "associations": ["Is-a means something belongs to a category."]},
        ]},

        {"name": "has-a (possession and attributes)", "items": [
            {"content": "Has-a means something possesses or contains something else.",
             "associations": ["A car has-a engine means an engine is inside or part of a car."]},
            {"content": "A car has-a engine means an engine is inside or part of a car.",
             "associations": ["Has-a means something possesses or contains something else."]},
            {"content": "Has-a is different from is-a. A car has-a wheel but a car is not a wheel.",
             "associations": ["Is-a means belonging to a category. Has-a means possessing something."]},
            {"content": "Is-a means belonging to a category. Has-a means possessing something.",
             "associations": ["Has-a is different from is-a. A car has-a wheel but a car is not a wheel."]},
            {"content": "Properties are things something has. A ball has the property of being round.",
             "associations": ["Has-a means something possesses or contains something else."]},
            {"content": "Owns, contains, includes, and possesses all express has-a relationships.",
             "associations": ["Has-a means something possesses or contains something else."]},
        ]},

        {"name": "part-of (composition)", "items": [
            {"content": "Part-of means something is a component of a larger whole.",
             "associations": ["A wheel is part-of a car. A finger is part-of a hand."]},
            {"content": "A wheel is part-of a car. A finger is part-of a hand.",
             "associations": ["Part-of means something is a component of a larger whole."]},
            {"content": "Part-of is the reverse of has-a. If X has-a Y, then Y is part-of X.",
             "associations": ["A car has-a wheel. A wheel is part-of a car. Same relationship, different direction."]},
            {"content": "A car has-a wheel. A wheel is part-of a car. Same relationship, different direction.",
             "associations": ["Part-of is the reverse of has-a. If X has-a Y, then Y is part-of X."]},
            {"content": "The whole is made up of its parts. Remove a part and the whole changes.",
             "associations": ["Part-of means something is a component of a larger whole."]},
            {"content": "Component, piece, element, and member all express part-of relationships.",
             "associations": ["Part-of means something is a component of a larger whole."]},
        ]},

        {"name": "causes (causation)", "items": [
            {"content": "Causes means one thing makes another thing happen.",
             "associations": ["Heat causes ice to melt. The heat is the cause, the melting is the effect."]},
            {"content": "Heat causes ice to melt. The heat is the cause, the melting is the effect.",
             "associations": ["Causes means one thing makes another thing happen."]},
            {"content": "The cause comes before the effect. The effect comes after the cause.",
             "associations": ["Causes means one thing makes another thing happen."]},
            {"content": "Makes, leads-to, results-in, and produces all express causes relationships.",
             "associations": ["Causes means one thing makes another thing happen."]},
            {"content": "A cause without its effect means something prevented it.",
             "associations": ["Prevents means stopping a cause from producing its effect."]},
            {"content": "Prevents means stopping a cause from producing its effect.",
             "associations": ["A cause without its effect means something prevented it."]},
            {"content": "Correlation is not causation. Two things happening together does not mean one caused the other.",
             "associations": ["Causes means one thing makes another thing happen."]},
        ]},

        {"name": "precedes (sequence and order)", "items": [
            {"content": "Precedes means one thing comes before another in order.",
             "associations": ["Before means earlier in sequence. After means later in sequence."]},
            {"content": "Before means earlier in sequence. After means later in sequence.",
             "associations": ["Precedes means one thing comes before another in order."]},
            {"content": "First means at the beginning. Last means at the end.",
             "associations": ["Between means after the first and before the last."]},
            {"content": "Between means after the first and before the last.",
             "associations": ["First means at the beginning. Last means at the end."]},
            {"content": "Next means the one that immediately follows. Previous means the one that came just before.",
             "associations": ["Precedes means one thing comes before another in order."]},
            {"content": "A sequence is a set of things arranged in order.",
             "associations": ["Precedes means one thing comes before another in order."]},
        ]},

        {"name": "contradicts (opposition)", "items": [
            {"content": "Contradicts means two things cannot both be true.",
             "associations": ["Alive contradicts dead. Something cannot be both alive and dead."]},
            {"content": "Alive contradicts dead. Something cannot be both alive and dead.",
             "associations": ["Contradicts means two things cannot both be true."]},
            {"content": "Opposite means as far apart as possible. Hot is the opposite of cold.",
             "associations": ["Contradicts means two things cannot both be true."]},
            {"content": "Conflicts-with, opposes, and is-incompatible-with all express contradiction.",
             "associations": ["Contradicts means two things cannot both be true."]},
            {"content": "If X contradicts Y, then knowing X is true tells you Y is false.",
             "associations": ["Contradicts means two things cannot both be true."]},
        ]},

        {"name": "same-as and different-from (equivalence)", "items": [
            {"content": "Same-as means two things are equal or identical.",
             "associations": ["Different-from means two things are not equal."]},
            {"content": "Different-from means two things are not equal.",
             "associations": ["Same-as means two things are equal or identical."]},
            {"content": "Equal means the same in value or amount.",
             "associations": ["Same-as means two things are equal or identical."]},
            {"content": "Similar means partly the same but not identical.",
             "associations": ["Similar is between same and different."]},
            {"content": "Similar is between same and different.",
             "associations": ["Similar means partly the same but not identical."]},
            {"content": "Synonym means two words that mean the same thing.",
             "associations": ["Antonym means two words that mean opposite things."]},
            {"content": "Antonym means two words that mean opposite things.",
             "associations": ["Synonym means two words that mean the same thing."]},
        ]},

        # ── Tense (time in language) ──
        {"name": "tense - past present future", "items": [
            {"content": "Past means something already happened. It is finished.",
             "associations": ["The ice melted means the melting already happened."]},
            {"content": "The ice melted means the melting already happened.",
             "associations": ["Past means something already happened. It is finished."]},
            {"content": "Present means something is happening right now.",
             "associations": ["The ice is melting means the melting is happening now."]},
            {"content": "The ice is melting means the melting is happening now.",
             "associations": ["Present means something is happening right now."]},
            {"content": "Future means something has not happened yet but will or might.",
             "associations": ["The ice will melt means the melting has not happened yet."]},
            {"content": "The ice will melt means the melting has not happened yet.",
             "associations": ["Future means something has not happened yet but will or might."]},
            {"content": "Was and were indicate past. Is and are indicate present. Will indicates future.",
             "associations": ["Tense tells you when something happens relative to now."]},
            {"content": "Tense tells you when something happens relative to now.",
             "associations": ["Past is before now. Present is now. Future is after now."]},
            {"content": "Past is before now. Present is now. Future is after now.",
             "associations": ["Tense tells you when something happens relative to now."]},
            {"content": "Already means in the past. Currently means in the present. Soon means in the near future.",
             "associations": ["Was and were indicate past. Is and are indicate present. Will indicates future."]},
            {"content": "The past cannot be changed. The future is not yet determined.",
             "associations": ["The present is the boundary between past and future."]},
            {"content": "The present is the boundary between past and future.",
             "associations": ["The past cannot be changed. The future is not yet determined."]},
        ]},

        # ── Pronouns and Reference ──
        {"name": "pronouns and reference", "items": [
            {"content": "A pronoun is a word that stands in for another word.",
             "associations": ["He, she, it, they, this, and that are all pronouns."]},
            {"content": "He, she, it, they, this, and that are all pronouns.",
             "associations": ["A pronoun is a word that stands in for another word."]},
            {"content": "He refers to a male person. She refers to a female person.",
             "associations": ["It refers to a thing or animal. They refers to multiple people or things."]},
            {"content": "It refers to a thing or animal. They refers to multiple people or things.",
             "associations": ["He refers to a male person. She refers to a female person."]},
            {"content": "This refers to something nearby or just mentioned. That refers to something farther away or mentioned earlier.",
             "associations": ["This and that are used to point to specific things."]},
            {"content": "This and that are used to point to specific things.",
             "associations": ["This refers to something nearby or just mentioned. That refers to something farther away or mentioned earlier."]},
            {"content": "The referent is what a pronoun points to. In John ran because he was late, he refers to John.",
             "associations": ["A pronoun is a word that stands in for another word."]},
            {"content": "To understand a pronoun, you must know what it refers to from context.",
             "associations": ["The referent is what a pronoun points to. In John ran because he was late, he refers to John."]},
            {"content": "I refers to the speaker. You refers to the listener. We refers to the speaker and others.",
             "associations": ["A pronoun is a word that stands in for another word."]},
        ]},

        # ── Negation in Language ──
        {"name": "negation in language", "items": [
            {"content": "Not placed in a sentence reverses the meaning of the verb.",
             "associations": ["The dog is running versus the dog is not running. Not reverses the action."]},
            {"content": "The dog is running versus the dog is not running. Not reverses the action.",
             "associations": ["Not placed in a sentence reverses the meaning of the verb."]},
            {"content": "No before a noun means zero of that thing. No dogs means zero dogs.",
             "associations": ["None means not any. Nothing means not anything."]},
            {"content": "None means not any. Nothing means not anything.",
             "associations": ["No before a noun means zero of that thing. No dogs means zero dogs."]},
            {"content": "Never means at no time. Nowhere means at no place.",
             "associations": ["Never, nowhere, nothing, and nobody are all negation words."]},
            {"content": "Never, nowhere, nothing, and nobody are all negation words.",
             "associations": ["All negation words connect back to the concept of not."]},
            {"content": "All negation words connect back to the concept of not.",
             "associations": ["Not reverses truth to false and false to true."]},
            {"content": "Un- and in- at the start of a word often mean not. Unhappy means not happy. Incorrect means not correct.",
             "associations": ["Prefixes can negate a word without using a separate not."]},
            {"content": "Prefixes can negate a word without using a separate not.",
             "associations": ["Un- and in- at the start of a word often mean not. Unhappy means not happy. Incorrect means not correct."]},
            {"content": "A double negative like not unhappy means the negation is reversed back, so it means somewhat happy.",
             "associations": ["Two nots cancel each other out, just like in logic."]},
            {"content": "Two nots cancel each other out, just like in logic.",
             "associations": ["Not-true is false. Not-false is true. Not-not-true is true."]},
        ]},
    ]
}


# ═══════════════════════════════════════════════════════════
# PHASE 3: NUMBER CONCEPTS
# Not arithmetic yet. What numbers ARE.
# ═══════════════════════════════════════════════════════════

PHASE_3 = {
    "name": "Number Concepts",
    "groups": [
        {"name": "quantity and counting", "items": [
            {"content": "A quantity is how much or how many of something there is.",
             "associations": ["Counting is finding the quantity of things in a group."]},
            {"content": "Counting is finding the quantity of things in a group.",
             "associations": ["A quantity is how much or how many of something there is."]},
            {"content": "A number is a way to represent a quantity.",
             "associations": ["Numbers let us talk about how many without listing each thing."]},
            {"content": "Numbers let us talk about how many without listing each thing.",
             "associations": ["A number is a way to represent a quantity."]},
            {"content": "Each number is exactly one more than the number before it.",
             "associations": ["Counting goes in order: each step adds exactly one."]},
            {"content": "Counting goes in order: each step adds exactly one.",
             "associations": ["Each number is exactly one more than the number before it."]},
        ]},

        {"name": "zero and one", "items": [
            {"content": "Zero means nothing, none, the absence of quantity.",
             "associations": ["Zero is the number before one."]},
            {"content": "Zero is the number before one.",
             "associations": ["One is the number after zero."]},
            {"content": "One is the number after zero.",
             "associations": ["One means a single thing exists."]},
            {"content": "One means a single thing exists.",
             "associations": ["Zero means nothing exists. One means something exists."]},
            {"content": "Zero means nothing exists. One means something exists.",
             "associations": ["The difference between zero and one is the difference between nothing and something."]},
            {"content": "The difference between zero and one is the difference between nothing and something.",
             "associations": ["Zero means nothing, none, the absence of quantity."]},
            {"content": "Empty means containing zero things.",
             "associations": ["Zero means nothing, none, the absence of quantity."]},
        ]},

        {"name": "more less and equal", "items": [
            {"content": "More means a greater quantity. Less means a smaller quantity.",
             "associations": ["More is the opposite of less."]},
            {"content": "More is the opposite of less.",
             "associations": ["More means a greater quantity. Less means a smaller quantity."]},
            {"content": "Equal means the same quantity. Two groups are equal if they have the same count.",
             "associations": ["Not-equal means different quantities."]},
            {"content": "Not-equal means different quantities.",
             "associations": ["If not equal, one must be more and the other must be less."]},
            {"content": "If not equal, one must be more and the other must be less.",
             "associations": ["More means a greater quantity. Less means a smaller quantity."]},
            {"content": "Greater-than means more. Less-than means fewer.",
             "associations": ["More means a greater quantity. Less means a smaller quantity."]},
            {"content": "Bigger, larger, and greater all mean more. Smaller, fewer, and lesser all mean less.",
             "associations": ["More is the opposite of less."]},
        ]},

        {"name": "basic number identities", "items": [
            {"content": "Two is one and one. Two means a pair.",
             "associations": ["Three is one more than two."]},
            {"content": "Three is one more than two.",
             "associations": ["Four is one more than three."]},
            {"content": "Four is one more than three.",
             "associations": ["Five is one more than four."]},
            {"content": "Five is one more than four.",
             "associations": ["Five fingers make one hand."]},
            {"content": "Five fingers make one hand.",
             "associations": ["Ten is two fives. Ten fingers make two hands."]},
            {"content": "Ten is two fives. Ten fingers make two hands.",
             "associations": ["Ten is the base of our counting system."]},
            {"content": "Ten is the base of our counting system.",
             "associations": ["After nine comes ten. After ninety-nine comes one hundred."]},
            {"content": "After nine comes ten. After ninety-nine comes one hundred.",
             "associations": ["One hundred is ten tens."]},
            {"content": "One hundred is ten tens.",
             "associations": ["One thousand is ten hundreds."]},
            {"content": "One thousand is ten hundreds.",
             "associations": ["Each step up multiplies by ten."]},
        ]},

        {"name": "adding and removing", "items": [
            {"content": "Adding means putting more together. The total gets bigger.",
             "associations": ["Removing means taking some away. The total gets smaller."]},
            {"content": "Removing means taking some away. The total gets smaller.",
             "associations": ["Adding means putting more together. The total gets bigger."]},
            {"content": "Adding zero changes nothing. Removing zero changes nothing.",
             "associations": ["Zero means nothing, none, the absence of quantity."]},
            {"content": "Adding is the same in any order. Three plus two equals two plus three.",
             "associations": ["Adding means putting more together. The total gets bigger."]},
            {"content": "If you add something and then remove the same amount, you are back where you started.",
             "associations": ["Adding and removing are opposites that undo each other."]},
            {"content": "Adding and removing are opposites that undo each other.",
             "associations": ["Adding means putting more together. The total gets bigger."]},
        ]},

        {"name": "grouping and multiplying", "items": [
            {"content": "A group is a collection of things treated as one unit.",
             "associations": ["Groups can contain different quantities of things."]},
            {"content": "Groups can contain different quantities of things.",
             "associations": ["Three groups of two means two plus two plus two, which is six."]},
            {"content": "Three groups of two means two plus two plus two, which is six.",
             "associations": ["Multiplying is counting equal groups."]},
            {"content": "Multiplying is counting equal groups.",
             "associations": ["Dividing is splitting into equal groups."]},
            {"content": "Dividing is splitting into equal groups.",
             "associations": ["Six divided into three groups gives two in each group."]},
            {"content": "Six divided into three groups gives two in each group.",
             "associations": ["Multiplying and dividing are opposites that undo each other."]},
            {"content": "Multiplying and dividing are opposites that undo each other.",
             "associations": ["Adding and removing are opposites that undo each other."]},
            {"content": "Half means divided into two equal parts.",
             "associations": ["Double means multiplied by two."]},
            {"content": "Double means multiplied by two.",
             "associations": ["Half means divided into two equal parts."]},
        ]},
    ]
}


# ═══════════════════════════════════════════════════════════
# PHASE 4: LANGUAGE STRUCTURE
# What kinds of sentences exist and what they do.
# ═══════════════════════════════════════════════════════════

PHASE_4 = {
    "name": "Language Structure",
    "groups": [
        {"name": "statement vs question vs command", "items": [
            {"content": "A statement declares that something is true or false.",
             "associations": ["The sky is blue is a statement."]},
            {"content": "The sky is blue is a statement.",
             "associations": ["A statement declares that something is true or false."]},
            {"content": "A question asks for information. A question expects an answer.",
             "associations": ["What color is the sky is a question."]},
            {"content": "What color is the sky is a question.",
             "associations": ["A question asks for information. A question expects an answer."]},
            {"content": "A command tells someone to do something.",
             "associations": ["Close the door is a command."]},
            {"content": "Close the door is a command.",
             "associations": ["A command tells someone to do something."]},
            {"content": "A request is a polite command. Please close the door is a request.",
             "associations": ["A command tells someone to do something."]},
            {"content": "Statements give information. Questions seek information. Commands request action.",
             "associations": ["These are the three basic purposes of language."]},
            {"content": "These are the three basic purposes of language.",
             "associations": ["Statements give information. Questions seek information. Commands request action."]},
        ]},

        {"name": "types of questions", "items": [
            {"content": "A what-question asks for identification or definition.",
             "associations": ["What is water asks for a definition of water."]},
            {"content": "What is water asks for a definition of water.",
             "associations": ["A what-question asks for identification or definition."]},
            {"content": "A why-question asks for a reason or cause.",
             "associations": ["Why does ice melt asks for the cause of melting."]},
            {"content": "Why does ice melt asks for the cause of melting.",
             "associations": ["A why-question asks for a reason or cause."]},
            {"content": "A how-question asks for a method or process.",
             "associations": ["How do birds fly asks for the method of flight."]},
            {"content": "How do birds fly asks for the method of flight.",
             "associations": ["A how-question asks for a method or process."]},
            {"content": "A where-question asks for a location.",
             "associations": ["A when-question asks for a time."]},
            {"content": "A when-question asks for a time.",
             "associations": ["A where-question asks for a location."]},
            {"content": "A who-question asks for a person or agent.",
             "associations": ["A what-question asks for a thing. A who-question asks for a person."]},
            {"content": "A yes-no question expects either yes or no as an answer.",
             "associations": ["Is water wet is a yes-no question. The answer is yes."]},
            {"content": "Is water wet is a yes-no question. The answer is yes.",
             "associations": ["A yes-no question expects either yes or no as an answer."]},
        ]},

        {"name": "subject and predicate", "items": [
            {"content": "Every statement has a subject and a predicate.",
             "associations": ["The subject is what the statement is about. The predicate says something about the subject."]},
            {"content": "The subject is what the statement is about. The predicate says something about the subject.",
             "associations": ["In the dog runs, dog is the subject and runs is the predicate."]},
            {"content": "In the dog runs, dog is the subject and runs is the predicate.",
             "associations": ["Every statement has a subject and a predicate."]},
            {"content": "The subject does the action. The object receives the action.",
             "associations": ["In the cat chased the mouse, cat is subject and mouse is object."]},
            {"content": "In the cat chased the mouse, cat is subject and mouse is object.",
             "associations": ["The subject does the action. The object receives the action."]},
        ]},

        {"name": "meaning and reference", "items": [
            {"content": "A word is a symbol that refers to something.",
             "associations": ["The word dog refers to a type of animal."]},
            {"content": "The word dog refers to a type of animal.",
             "associations": ["A word is a symbol that refers to something."]},
            {"content": "A definition explains what a word means.",
             "associations": ["Asking what does X mean is asking for a definition."]},
            {"content": "Asking what does X mean is asking for a definition.",
             "associations": ["A definition explains what a word means."]},
            {"content": "Context changes what a word means. Bank means different things near a river versus near money.",
             "associations": ["The same word can have different meanings in different situations."]},
            {"content": "The same word can have different meanings in different situations.",
             "associations": ["Context changes what a word means. Bank means different things near a river versus near money."]},
            {"content": "About means the topic or subject of discussion. This is about dogs means dogs is the topic.",
             "associations": ["When someone asks about something, they want information on that topic."]},
            {"content": "When someone asks about something, they want information on that topic.",
             "associations": ["About means the topic or subject of discussion. This is about dogs means dogs is the topic."]},
        ]},

        {"name": "connecting ideas", "items": [
            {"content": "And connects things that are both true or both included.",
             "associations": ["But connects things that contrast or seem surprising together."]},
            {"content": "But connects things that contrast or seem surprising together.",
             "associations": ["It is small but strong means the strength is surprising given the size."]},
            {"content": "It is small but strong means the strength is surprising given the size.",
             "associations": ["But connects things that contrast or seem surprising together."]},
            {"content": "Because connects an effect to its cause.",
             "associations": ["I am wet because it rained. The rain caused the wetness."]},
            {"content": "I am wet because it rained. The rain caused the wetness.",
             "associations": ["Because connects an effect to its cause."]},
            {"content": "So connects a cause to its effect.",
             "associations": ["It rained, so I am wet. Same facts as because but in opposite order."]},
            {"content": "It rained, so I am wet. Same facts as because but in opposite order.",
             "associations": ["So connects a cause to its effect."]},
            {"content": "Although and even though introduce something that does not prevent the main point.",
             "associations": ["Although it rained, I went outside. The rain did not stop me."]},
            {"content": "Although it rained, I went outside. The rain did not stop me.",
             "associations": ["Although and even though introduce something that does not prevent the main point."]},
        ]},
    ]
}


# ═══════════════════════════════════════════════════════════
# RUNNER
# ═══════════════════════════════════════════════════════════

ALL_PHASES = [PHASE_1, PHASE_1_5, PHASE_2, PHASE_3, PHASE_4]


def kill_existing_mcp():
    try:
        result = subprocess.run(
            ["taskkill", "/F", "/IM", "ai_factory_mcp.exe"],
            capture_output=True, text=True
        )
        if result.returncode == 0:
            print("[foundation] Killed existing MCP process(es)")
            time.sleep(1)
    except Exception:
        pass


def main():
    parser = argparse.ArgumentParser(description="Foundation Primitives for Lu")
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--phase", type=int, default=0,
                        help="Run only phase N (1-4). 0=all")
    args = parser.parse_args()

    phases = ALL_PHASES
    if args.phase > 0:
        if args.phase > len(ALL_PHASES):
            print(f"Phase {args.phase} does not exist. Max: {len(ALL_PHASES)}")
            return
        phases = [ALL_PHASES[args.phase - 1]]

    # Count total items
    total_items = 0
    for phase in phases:
        for group in phase["groups"]:
            total_items += len(group["items"])

    print(f"Lu's Foundation Training")
    print(f"Phases: {len(phases)}")
    print(f"Total atoms: {total_items}")
    print()

    if args.dry_run:
        for phase in phases:
            print(f"  [{phase['name']}]")
            for group in phase["groups"]:
                print(f"    {group['name']}: {len(group['items'])} atoms")
                for item in group["items"]:
                    assoc = len(item.get("associations", []))
                    print(f"      - {item['content'][:70]}... ({assoc} assoc)")
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

    for phase in phases:
        print(f"\n--- {phase['name']} ---")
        for group in phase["groups"]:
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
    print(f"Foundation training complete for Lu!")
    print(f"Time: {elapsed:.1f}s")
    print(f"Atoms learned: {total_learned}")
    print(f"Bonds created: {total_bonds}")
    print(f"Brain: {start_nodes} -> {final_nodes} (+{final_nodes - start_nodes})")
    print(f"{'='*50}")

    bridge.stop()


if __name__ == "__main__":
    main()
