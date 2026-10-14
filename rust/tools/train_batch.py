#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright (C) 2026 DialectForge LLC
"""
Lu Ai — Foundation Training

Builds the brain's ability to COMMUNICATE and REASON, not just store facts.

Categories:
  1. Word Relationships — synonyms, antonyms, categories, parts
  2. Sentence Patterns — how ideas connect, grammar, structure
  3. Reasoning — cause/effect, if/then, comparisons, opinions
  4. Conversational — questions/answers, context, appropriate responses
  5. Concrete Math — real number facts, not descriptions of math
  6. Emotional/Social — feelings, intentions, social situations
  7. Common Sense — everyday knowledge people just "know"
  8. General Knowledge — but taught in RELATIONAL way, not just facts

Usage:
    python train_batch.py
    python train_batch.py --start-from 50
    python train_batch.py --dry-run
    python train_batch.py --model mistral
"""

import sys
import os
import time
import subprocess
import argparse

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from train import MCPBridge, generate_knowledge, DEFAULT_MODEL

# Each topic has a name AND a custom prompt hint that tells Ollama
# what KIND of knowledge to generate (not just encyclopedia facts)
TOPICS = [
    # ══════════════════════════════════════════════════════
    # 1. WORD RELATIONSHIPS — the glue of language
    # ══════════════════════════════════════════════════════
    # These teach the brain how words RELATE, not what they mean
    {"name": "synonyms for common verbs",
     "prompt": "Generate pairs of synonyms for common English verbs. Each fact should say 'X means the same as Y' or 'X is another way to say Y'. Examples: 'Begin means the same as start.' 'Finish is another way to say complete.'"},
    {"name": "synonyms for common adjectives",
     "prompt": "Generate pairs of synonyms for common English adjectives. Format: 'X means the same as Y'. Examples: 'Happy means the same as joyful.' 'Big is another way to say large.'"},
    {"name": "synonyms for common nouns",
     "prompt": "Generate pairs of synonyms for common English nouns. Format: 'A X is the same as a Y'. Examples: 'A car is the same as an automobile.' 'A kid is the same as a child.'"},
    {"name": "antonyms for common words",
     "prompt": "Generate pairs of antonyms (opposites). Format: 'X is the opposite of Y'. Examples: 'Hot is the opposite of cold.' 'Up is the opposite of down.' 'Fast is the opposite of slow.'"},
    {"name": "antonyms for feelings and states",
     "prompt": "Generate pairs of opposite feelings and states. Format: 'X is the opposite of Y'. Examples: 'Happy is the opposite of sad.' 'Calm is the opposite of anxious.' 'Brave is the opposite of afraid.'"},
    {"name": "categories of animals",
     "prompt": "Generate facts about what category animals belong to. Format: 'A X is a type of Y'. Examples: 'A dog is a type of mammal.' 'A robin is a type of bird.' 'A salmon is a type of fish.'"},
    {"name": "categories of food",
     "prompt": "Generate facts about what category foods belong to. Format: 'X is a type of Y'. Examples: 'An apple is a type of fruit.' 'Broccoli is a type of vegetable.' 'Salmon is a type of seafood.'"},
    {"name": "categories of objects",
     "prompt": "Generate facts about what category everyday objects belong to. Format: 'A X is a type of Y'. Examples: 'A hammer is a type of tool.' 'A chair is a type of furniture.' 'A shirt is a type of clothing.'"},
    {"name": "part-whole relationships body",
     "prompt": "Generate facts about parts of things. Format: 'A X is part of a Y' or 'A Y has a X'. Examples: 'A finger is part of a hand.' 'A wheel is part of a car.' 'A roof is part of a house.'"},
    {"name": "part-whole relationships objects",
     "prompt": "Generate facts about parts of everyday objects. Format: 'A X is part of a Y'. Examples: 'A blade is part of a knife.' 'A screen is part of a phone.' 'A page is part of a book.'"},
    {"name": "material relationships",
     "prompt": "Generate facts about what things are made of. Format: 'X is made of Y' or 'X is made from Y'. Examples: 'Paper is made from wood.' 'Glass is made from sand.' 'Bread is made from flour.'"},
    {"name": "location relationships",
     "prompt": "Generate facts about where things are found. Format: 'You find X in a Y' or 'X is usually in a Y'. Examples: 'You find books in a library.' 'Fish live in water.' 'A pilot works in a cockpit.'"},
    {"name": "tool-purpose relationships",
     "prompt": "Generate facts about what tools are used for. Format: 'A X is used for Y' or 'You use a X to Y'. Examples: 'A knife is used for cutting.' 'You use a broom to sweep.' 'A thermometer is used for measuring temperature.'"},
    {"name": "size comparisons",
     "prompt": "Generate facts comparing sizes of things. Format: 'X is bigger/smaller than Y'. Examples: 'An elephant is bigger than a dog.' 'A grape is smaller than an apple.' 'The ocean is bigger than a lake.'"},
    {"name": "speed comparisons",
     "prompt": "Generate facts comparing speeds. Format: 'X is faster/slower than Y'. Examples: 'A cheetah is faster than a horse.' 'Walking is slower than running.' 'Light is faster than sound.'"},
    {"name": "age and time words",
     "prompt": "Generate facts about time-related word meanings. Format: 'X means Y'. Examples: 'Ancient means very very old.' 'Brief means lasting a short time.' 'A decade is ten years.' 'A century is one hundred years.'"},

    # ══════════════════════════════════════════════════════
    # 2. SENTENCE PATTERNS — how ideas connect
    # ══════════════════════════════════════════════════════
    {"name": "cause and effect with because",
     "prompt": "Generate simple cause-and-effect sentences using 'because'. Format: 'X because Y'. Examples: 'Ice melts because heat causes it to change from solid to liquid.' 'Plants need sunlight because they use it to make food.' 'People wear coats because cold weather can make them uncomfortable.'"},
    {"name": "cause and effect with so and therefore",
     "prompt": "Generate cause-and-effect sentences using 'so' or 'therefore'. Format: 'X, so Y' or 'X, therefore Y'. Examples: 'It was raining, so we brought umbrellas.' 'The battery died, so the phone turned off.' 'She studied hard, therefore she passed the exam.'"},
    {"name": "if-then conditional statements",
     "prompt": "Generate if-then conditional statements about everyday situations. Format: 'If X, then Y'. Examples: 'If you heat water to 100 degrees Celsius, then it boils.' 'If you do not sleep enough, then you feel tired.' 'If it rains and you have no umbrella, then you get wet.'"},
    {"name": "contrast statements with but and however",
     "prompt": "Generate contrast sentences using 'but' or 'however'. Format: 'X, but Y' or 'X. However, Y'. Examples: 'Dogs are loyal, but they need a lot of attention.' 'Gold is valuable, but it is too soft for building.' 'The sun is a star. However, it looks different from other stars because it is so close.'"},
    {"name": "sequence and time connectors",
     "prompt": "Generate sentences showing sequence using 'first, then, finally' or 'before, after, while'. Examples: 'First you crack the egg, then you mix it, finally you cook it.' 'Before you drive, you must learn the rules.' 'After the rain stops, the ground stays wet for a while.'"},
    {"name": "comparison sentences",
     "prompt": "Generate comparison sentences using 'like', 'similar to', 'different from', 'compared to'. Examples: 'A brain is like a computer that processes information.' 'Frogs are similar to toads, but toads have drier skin.' 'The moon is different from a star because it does not produce its own light.'"},
    {"name": "purpose statements with to and for",
     "prompt": "Generate purpose sentences using 'to' or 'for' or 'in order to'. Examples: 'People exercise to stay healthy.' 'A bridge is built for crossing over water or roads.' 'Animals hibernate in order to survive the winter when food is scarce.'"},
    {"name": "example and illustration sentences",
     "prompt": "Generate sentences that give examples using 'for example', 'such as', 'for instance'. Examples: 'Some mammals can fly, for example bats.' 'Citrus fruits such as oranges and lemons are rich in vitamin C.' 'Some metals are magnetic, for instance iron and nickel.'"},
    {"name": "degree and intensity words",
     "prompt": "Generate sentences showing degree using 'very', 'slightly', 'extremely', 'somewhat', 'barely'. Examples: 'The sun is extremely hot compared to the earth.' 'Ice cream is slightly sweet, while candy is very sweet.' 'Mercury is barely larger than the moon.'"},
    {"name": "negation and exception patterns",
     "prompt": "Generate sentences with negation or exceptions. Format: 'Not all X are Y' or 'X except Y'. Examples: 'Not all birds can fly, for example penguins and ostriches cannot.' 'All planets orbit the sun, but not all have moons.' 'Most metals are solid at room temperature, except mercury which is liquid.'"},

    # ══════════════════════════════════════════════════════
    # 3. REASONING AND LOGIC
    # ══════════════════════════════════════════════════════
    {"name": "logical deductions",
     "prompt": "Generate simple logical deduction chains. Format: 'All X are Y. Z is an X. Therefore Z is Y.' Examples: 'All mammals breathe air. A whale is a mammal. Therefore a whale breathes air.' 'All squares have four equal sides. This shape has four equal sides and right angles. Therefore this shape is a square.'"},
    {"name": "analogies and relationships",
     "prompt": "Generate analogy relationships. Format: 'X is to Y as A is to B'. Examples: 'Hot is to cold as light is to dark.' 'Puppy is to dog as kitten is to cat.' 'Page is to book as brick is to wall.' 'Doctor is to hospital as teacher is to school.'"},
    {"name": "what happens when - physical world",
     "prompt": "Generate cause-effect facts about the physical world. Format: 'When X happens, Y happens' or 'If you X, then Y'. Examples: 'When you drop a glass on a hard floor, it breaks.' 'If you leave ice cream in the sun, it melts.' 'When you mix red and blue paint, you get purple.'"},
    {"name": "what happens when - social situations",
     "prompt": "Generate cause-effect facts about social situations. Format: 'When you X, people usually Y'. Examples: 'When you say thank you, people feel appreciated.' 'When someone is crying, they are usually sad or hurt.' 'When you help someone, they often want to help you back.' 'When you break a promise, people lose trust in you.'"},
    {"name": "why questions and answers",
     "prompt": "Generate why-because pairs. Format: 'Why does X? Because Y.' Examples: 'Why does ice float on water? Because ice is less dense than liquid water.' 'Why do leaves change color in autumn? Because trees stop making chlorophyll.' 'Why do we yawn? Because the body needs more oxygen or the brain needs to cool down.'"},
    {"name": "how things work simply",
     "prompt": "Generate simple explanations of how everyday things work. Format: 'X works by Y'. Examples: 'A refrigerator works by removing heat from inside and releasing it outside.' 'Soap works by breaking up oils and dirt so water can wash them away.' 'A mirror works by reflecting light back to your eyes.'"},
    {"name": "predictions and expectations",
     "prompt": "Generate prediction sentences. Format: 'If X continues, then probably Y' or 'When X usually Y follows'. Examples: 'If dark clouds gather, then probably it will rain soon.' 'When a dog wags its tail, usually it is happy or excited.' 'If you practice something every day, you probably get better at it over time.'"},
    {"name": "generalizations and rules",
     "prompt": "Generate common generalizations and rules of thumb. Format: 'Generally X' or 'Most X tend to Y'. Examples: 'Generally, larger animals live longer than smaller ones.' 'Most birds can fly, but some like penguins and ostriches cannot.' 'Warm air generally rises while cold air sinks.'"},
    {"name": "problem and solution patterns",
     "prompt": "Generate problem-solution pairs. Format: 'Problem: X. Solution: Y.' Examples: 'Problem: The room is too dark. Solution: Turn on a light or open the curtains.' 'Problem: You are lost. Solution: Use a map, ask for directions, or retrace your steps.' 'Problem: Food spoils quickly. Solution: Store it in a refrigerator to slow down bacteria growth.'"},
    {"name": "opinion and perspective patterns",
     "prompt": "Generate sentences that express opinions or perspectives. Format: 'Some people think X because Y, while others think A because B'. Examples: 'Some people think dogs are better pets because they are loyal, while others prefer cats because they are independent.' 'Some people enjoy rainy days because they find rain calming, while others dislike rain because it limits outdoor activities.'"},

    # ══════════════════════════════════════════════════════
    # 4. CONVERSATIONAL KNOWLEDGE
    # ══════════════════════════════════════════════════════
    {"name": "question types and how to answer them",
     "prompt": "Generate facts about how different types of questions should be answered. Examples: 'A yes or no question expects either yes or no as an answer.' 'A why question expects a reason or explanation as an answer.' 'A how question expects a method or process as an answer.' 'A what question expects a definition or identification as an answer.'"},
    {"name": "greetings and social phrases",
     "prompt": "Generate facts about common social phrases and what they mean. Examples: 'Hello and hi are common greetings used when meeting someone.' 'Goodbye and see you later are phrases used when leaving.' 'Please is used to make a request more polite.' 'Thank you is used to show appreciation after receiving help.' 'How are you is a common greeting that asks about someone's wellbeing.'"},
    {"name": "expressing agreement and disagreement",
     "prompt": "Generate facts about how people express agreement and disagreement in conversation. Examples: 'Saying I agree means you share the same opinion.' 'Saying I see your point but means you understand but disagree.' 'Nodding usually means yes or agreement in many cultures.' 'That makes sense is a way to show you understand and agree.'"},
    {"name": "asking for clarification",
     "prompt": "Generate facts about phrases used to ask for clarification. Examples: 'What do you mean by that is a way to ask someone to explain further.' 'Could you give an example means you want a concrete illustration.' 'In other words is used to rephrase something for clarity.' 'Can you be more specific asks someone to narrow down a broad statement.'"},
    {"name": "expressing uncertainty",
     "prompt": "Generate facts about how people express uncertainty. Examples: 'I think means the speaker is not completely sure.' 'Maybe and perhaps indicate something is possible but not certain.' 'I am not sure means the speaker lacks confidence in their answer.' 'It depends means the answer changes based on the situation.' 'Probably means something is likely but not guaranteed.'"},
    {"name": "responding to feelings",
     "prompt": "Generate facts about appropriate responses to different emotions. Examples: 'When someone is sad, saying I am sorry to hear that shows empathy.' 'When someone shares good news, saying congratulations shows you are happy for them.' 'When someone is frustrated, saying that sounds difficult validates their feelings.' 'When someone is scared, saying it will be okay can be reassuring.'"},
    {"name": "topic transitions in conversation",
     "prompt": "Generate facts about how people change topics in conversation. Examples: 'By the way is used to introduce a new but related topic.' 'Speaking of X is used to transition to a topic related to what was just said.' 'That reminds me is used when something in the conversation triggers a new thought.' 'On a different note signals a complete change of topic.'"},
    {"name": "context and relevance in responses",
     "prompt": "Generate facts about what makes a good response in conversation. Examples: 'A good response addresses what was actually asked, not a different question.' 'Giving too much unrelated information makes a response confusing.' 'A response should match the tone of the question, casual question gets casual answer.' 'When someone asks for your opinion, they want your thoughts, not just facts.' 'Short questions usually expect short answers unless detail is requested.'"},

    # ══════════════════════════════════════════════════════
    # 5. CONCRETE MATH — actual facts, not descriptions
    # ══════════════════════════════════════════════════════
    {"name": "addition facts single digit",
     "prompt": "Generate concrete addition facts with single digit numbers. Each fact should be a complete sentence. Format: 'X plus Y equals Z'. Examples: 'Two plus three equals five.' 'Seven plus eight equals fifteen.' 'Nine plus six equals fifteen.' Use number words not digits."},
    {"name": "addition facts double digit",
     "prompt": "Generate concrete addition facts with double digit numbers. Format: 'X plus Y equals Z'. Examples: 'Twenty plus thirty equals fifty.' 'Fifteen plus twenty-five equals forty.' 'Forty-four plus eleven equals fifty-five.' Use number words not digits."},
    {"name": "subtraction facts",
     "prompt": "Generate concrete subtraction facts. Format: 'X minus Y equals Z'. Examples: 'Ten minus three equals seven.' 'Twenty minus eight equals twelve.' 'One hundred minus forty equals sixty.' Use number words not digits."},
    {"name": "multiplication facts",
     "prompt": "Generate concrete multiplication facts. Format: 'X times Y equals Z' or 'X multiplied by Y equals Z'. Examples: 'Three times four equals twelve.' 'Seven times eight equals fifty-six.' 'Six times nine equals fifty-four.' Use number words not digits."},
    {"name": "division facts",
     "prompt": "Generate concrete division facts. Format: 'X divided by Y equals Z'. Examples: 'Twelve divided by three equals four.' 'Twenty divided by five equals four.' 'Forty-two divided by seven equals six.' Use number words not digits."},
    {"name": "fractions in plain language",
     "prompt": "Generate facts about fractions using everyday language. Examples: 'Half of ten is five.' 'A quarter of a hundred is twenty-five.' 'One third of twelve is four.' 'Three quarters of an hour is forty-five minutes.' 'Half of a dollar is fifty cents.'"},
    {"name": "percentages in plain language",
     "prompt": "Generate facts about percentages using everyday language. Examples: 'Fifty percent means half.' 'Twenty-five percent means one quarter.' 'One hundred percent means all of it.' 'Ten percent of a hundred is ten.' 'If you score seventy percent on a test with a hundred questions, you got seventy right.'"},
    {"name": "number comparisons and ordering",
     "prompt": "Generate facts about number comparisons. Examples: 'One hundred is greater than fifty.' 'Negative numbers are less than zero.' 'A million is a thousand thousands.' 'A billion is a thousand millions.' 'Zero is neither positive nor negative.'"},
    {"name": "measurement and units",
     "prompt": "Generate facts about measurements and units in everyday language. Examples: 'There are twelve inches in one foot.' 'There are sixty minutes in one hour.' 'There are twenty-four hours in one day.' 'There are three hundred sixty-five days in one year.' 'One kilogram is about two point two pounds.' 'Water freezes at zero degrees Celsius or thirty-two degrees Fahrenheit.'"},
    {"name": "math word problems simple",
     "prompt": "Generate simple word problems with answers. Format: 'If X, then Y. The answer is Z.' Examples: 'If you have five apples and give away two, you have three apples left.' 'If a shirt costs twenty dollars and you pay with a fifty, your change is thirty dollars.' 'If you drive sixty miles per hour for two hours, you travel one hundred twenty miles.'"},
    {"name": "number words and digit mapping",
     "prompt": "Generate facts mapping number words to their digit forms. Examples: 'One is written as the digit 1.' 'Thirteen is written as 13.' 'A hundred is written as 100.' 'A thousand is written as 1000.' 'Forty-two is written as 42.' 'Zero is written as 0.'"},
    {"name": "math vocabulary",
     "prompt": "Generate definitions of basic math terms in simple language. Examples: 'Sum means the result of adding numbers together.' 'Product means the result of multiplying numbers together.' 'Difference means the result of subtracting one number from another.' 'Even numbers can be divided by two with no remainder.' 'Odd numbers have a remainder of one when divided by two.'"},

    # ══════════════════════════════════════════════════════
    # 6. EMOTIONAL AND SOCIAL INTELLIGENCE
    # ══════════════════════════════════════════════════════
    {"name": "basic emotions and their signs",
     "prompt": "Generate facts about emotions and how they show. Examples: 'When someone smiles, they are usually happy or friendly.' 'Crying often means someone is sad, hurt, or overwhelmed.' 'A frown usually means someone is unhappy or confused.' 'Laughter usually means something is funny or someone feels joyful.' 'Shaking or trembling can be a sign of fear or cold.'"},
    {"name": "intentions and motivations",
     "prompt": "Generate facts about why people do things. Examples: 'People eat because they are hungry and need energy.' 'People work to earn money to pay for things they need.' 'Children go to school to learn skills and knowledge.' 'People exercise to stay healthy and feel good.' 'People ask questions because they want to understand something.'"},
    {"name": "social norms and expectations",
     "prompt": "Generate facts about common social norms. Examples: 'In many cultures, it is polite to say please when asking for something.' 'Interrupting someone while they are speaking is generally considered rude.' 'Arriving on time shows respect for other people's time.' 'Saying sorry after making a mistake shows accountability.' 'Making eye contact during conversation shows you are paying attention.'"},
    {"name": "perspective taking",
     "prompt": "Generate facts about understanding different perspectives. Examples: 'Different people can have different opinions about the same thing and both can be valid.' 'What seems easy for one person might be hard for another.' 'A person's experiences shape how they see the world.' 'Before judging someone, it helps to understand their situation.' 'Two people can look at the same event and remember it differently.'"},
    {"name": "needs and wants",
     "prompt": "Generate facts about the difference between needs and wants. Examples: 'Food, water, and shelter are basic needs that all humans require.' 'A want is something you desire but do not need to survive.' 'A person needs clothing for warmth and protection but wants fashionable clothing.' 'Sleep is a need because the body cannot function without rest.' 'Entertainment is a want that makes life more enjoyable but is not essential.'"},

    # ══════════════════════════════════════════════════════
    # 7. COMMON SENSE — things everyone just "knows"
    # ══════════════════════════════════════════════════════
    {"name": "physical common sense",
     "prompt": "Generate common sense facts about the physical world that most people know. Examples: 'Heavy objects sink in water while light objects float.' 'Fire is hot and can burn you if you touch it.' 'Sharp objects like knives can cut you.' 'If you throw a ball up, it comes back down because of gravity.' 'Wet surfaces are more slippery than dry ones.'"},
    {"name": "time common sense",
     "prompt": "Generate common sense facts about time. Examples: 'Morning comes before afternoon, and afternoon comes before evening.' 'Yesterday was the day before today, and tomorrow is the day after today.' 'A year has four seasons: spring, summer, autumn, and winter.' 'People usually sleep at night and are awake during the day.' 'Breakfast is the first meal of the day, lunch is the midday meal, and dinner is the evening meal.'"},
    {"name": "spatial common sense",
     "prompt": "Generate common sense facts about space and position. Examples: 'The sky is above us and the ground is below us.' 'North is the opposite direction of south, and east is the opposite of west.' 'Things that are closer look bigger than things that are far away.' 'The inside of a house is usually warmer than the outside in winter.' 'You cannot walk through a solid wall.'"},
    {"name": "living things common sense",
     "prompt": "Generate common sense facts about living things. Examples: 'All living things need water to survive.' 'Animals need food for energy, just like people do.' 'Plants grow toward sunlight.' 'Babies grow into children, children grow into adults.' 'Dead things do not move, grow, or eat.'"},
    {"name": "everyday cause and effect",
     "prompt": "Generate obvious cause-and-effect facts that everyone knows. Examples: 'If you do not water a plant, it will eventually die.' 'If you touch something very hot, you will burn your hand.' 'If you eat too much, you will feel sick.' 'If you run fast for a long time, you will get tired.' 'If you leave food out for too long, it will spoil.'"},
    {"name": "what things are for",
     "prompt": "Generate facts about common objects and their purposes. Examples: 'A chair is for sitting on.' 'A bed is for sleeping in.' 'A cup is for drinking from.' 'A pen is for writing with.' 'A door is for entering and leaving a room.' 'A window lets light in and allows you to see outside.'"},
    {"name": "properties of common materials",
     "prompt": "Generate facts about properties of common materials. Examples: 'Metal is hard and conducts heat.' 'Wood is lighter than metal and can float on water.' 'Glass is transparent so you can see through it.' 'Rubber is flexible and bounces when you drop it.' 'Cotton is soft and absorbs water.' 'Ice is slippery and cold to touch.'"},
    {"name": "weather and nature common sense",
     "prompt": "Generate common sense facts about weather and nature. Examples: 'Rain falls from clouds and makes things wet.' 'Snow is frozen water that falls when it is very cold.' 'The sun provides light during the day.' 'Stars are visible at night but not during the day because sunlight is too bright.' 'Wind is air that is moving.'"},

    # ══════════════════════════════════════════════════════
    # 8. RELATIONAL GENERAL KNOWLEDGE
    # Not just facts — facts connected with WHY and HOW
    # ══════════════════════════════════════════════════════
    {"name": "space and why it matters",
     "prompt": "Generate facts about space that explain WHY things are the way they are, not just what they are. Examples: 'The sun is important because it provides the light and heat that makes life on Earth possible.' 'Space is dark because there is no atmosphere to scatter light.' 'Stars look small because they are extremely far away, but most are actually much larger than Earth.' 'We feel weightless in space because there is no ground pushing back against gravity.'"},
    {"name": "history and why it matters",
     "prompt": "Generate facts about history that explain WHY events mattered, not just what happened. Examples: 'The printing press mattered because it allowed ideas to spread quickly to many people.' 'Agriculture changed human life because it allowed people to stop moving around and build permanent settlements.' 'Writing was important because it allowed people to record knowledge and pass it to future generations.'"},
    {"name": "science connected to daily life",
     "prompt": "Generate facts about science that connect to everyday experiences. Examples: 'Your food gives you energy because your body breaks it down in a process similar to burning fuel.' 'You feel cold when wet because evaporating water takes heat from your skin.' 'Soap cleans your hands because it breaks up the oils that trap dirt and germs.' 'You see lightning before you hear thunder because light travels much faster than sound.'"},
    {"name": "geography connected to people",
     "prompt": "Generate facts about geography that explain how the land affects people's lives. Examples: 'People near oceans often eat more seafood because fish are easy to catch.' 'Mountain regions tend to be cooler because temperature drops as altitude increases.' 'Rivers have been important throughout history because they provide fresh water for drinking and farming.' 'Deserts have few people because water is scarce and temperatures are extreme.'"},
    {"name": "animals and how they relate to humans",
     "prompt": "Generate facts about animals that relate to human experience. Examples: 'Dogs were the first animals domesticated by humans, used for hunting and protection.' 'Bees are important to humans because they pollinate the plants that produce much of our food.' 'Horses changed human transportation and warfare for thousands of years.' 'Chickens are the most common bird in the world because humans raise billions of them for food.'"},
    {"name": "technology and how it changed life",
     "prompt": "Generate facts about technology that explain how it changed everyday life. Examples: 'Refrigerators changed food storage because people could keep food fresh for much longer.' 'The telephone changed communication because people could talk to others far away instantly.' 'Cars changed where people live because they could travel further to work each day.' 'The internet changed learning because anyone can access information from anywhere.'"},
    {"name": "the human body and why it does things",
     "prompt": "Generate facts about the human body that explain WHY it does things. Examples: 'You sneeze because your body is trying to clear irritants from your nose.' 'Your heart beats because it needs to pump blood carrying oxygen to every cell.' 'You feel pain because your nerves are warning your brain that something might be harmful.' 'You get thirsty because your body needs water to function and is telling you to drink.'"},
    {"name": "food and why we eat what we eat",
     "prompt": "Generate facts about food that explain WHY certain things are eaten. Examples: 'People eat breakfast because the body needs fuel after sleeping all night.' 'Fruits are healthy because they contain vitamins that help the body fight disease.' 'Cooking meat is important because heat kills bacteria that could make you sick.' 'Salt was historically valuable because it preserved food before refrigeration existed.'"},

    # ══════════════════════════════════════════════════════
    # 9. ABSTRACT CONCEPTS — ideas, not just objects
    # ══════════════════════════════════════════════════════
    {"name": "abstract concepts defined simply",
     "prompt": "Generate simple definitions of abstract concepts. Examples: 'Freedom means being able to make your own choices.' 'Justice means treating people fairly and equally.' 'Trust means believing someone will do what they say.' 'Courage means doing something even when you are afraid.' 'Patience means being willing to wait without getting angry.'"},
    {"name": "abstract vs concrete",
     "prompt": "Generate facts that distinguish abstract from concrete things. Examples: 'You can touch a rock but you cannot touch an idea.' 'Love is a feeling, not a physical object.' 'Money is physical but wealth is an abstract concept about how much you have.' 'A book is a concrete thing but the story inside it is abstract.' 'Temperature is abstract but you can feel hot and cold.'"},
    {"name": "qualities and traits",
     "prompt": "Generate facts about human qualities and what they mean. Examples: 'A generous person gives freely to others without expecting anything back.' 'A curious person wants to learn and understand new things.' 'A patient person can wait calmly without getting frustrated.' 'An honest person tells the truth even when it is difficult.' 'A creative person can imagine new ideas and solve problems in unusual ways.'"},
    {"name": "relationships between concepts",
     "prompt": "Generate facts about how abstract concepts relate to each other. Examples: 'Trust and honesty are connected because people trust those who are honest.' 'Fear and courage are related because courage means acting despite fear.' 'Knowledge and understanding are different because you can know facts without understanding why they matter.' 'Freedom and responsibility go together because being free to choose means being responsible for your choices.'"},
    {"name": "metaphors people commonly use",
     "prompt": "Generate common metaphors and what they mean. Examples: 'Time is money means time is valuable and should not be wasted.' 'The world is a stage means people play different roles in life.' 'Knowledge is power means understanding things gives you the ability to act effectively.' 'Life is a journey means life is a series of experiences that take you somewhere.' 'Burning bridges means damaging relationships so you cannot go back.'"},

    # ══════════════════════════════════════════════════════
    # 10. DEFINITIONS — core vocabulary the brain MUST know
    # ══════════════════════════════════════════════════════
    {"name": "what is questions about nature",
     "prompt": "Generate clear one-sentence definitions answering 'what is' questions. Examples: 'Water is a clear liquid that all living things need to survive.' 'Air is the invisible mixture of gases that surrounds the Earth and that we breathe.' 'Fire is a chemical reaction that produces heat and light when something burns.' 'A mountain is a very tall natural landform that rises high above the surrounding area.' 'An ocean is a vast body of salt water that covers most of the Earth.'"},
    {"name": "what is questions about human things",
     "prompt": "Generate clear one-sentence definitions of human-made concepts. Examples: 'A city is a large place where many people live and work together.' 'Money is something people use to exchange for goods and services.' 'A language is a system of words and rules that people use to communicate.' 'A law is a rule made by a government that people must follow.' 'Art is something created to express ideas, emotions, or beauty.'"},
    {"name": "what is questions about feelings and experiences",
     "prompt": "Generate clear one-sentence definitions of feelings and experiences. Examples: 'Happiness is a feeling of joy and contentment.' 'Loneliness is the feeling of being alone and wanting connection with others.' 'Curiosity is the desire to learn or know about something.' 'Boredom is the feeling of having nothing interesting to do.' 'Excitement is a feeling of eager enthusiasm about something.'"},
    {"name": "what is questions about actions",
     "prompt": "Generate clear one-sentence definitions of common actions. Examples: 'Learning is the process of gaining knowledge or skills through study or experience.' 'Teaching is the act of helping someone else understand something.' 'Playing is an activity done for enjoyment and fun.' 'Working is using effort to accomplish a task or earn a living.' 'Thinking is using your mind to consider ideas or solve problems.'"},

    # ══════════════════════════════════════════════════════
    # 11. MORE WORD RELATIONSHIPS — deeper vocabulary
    # ══════════════════════════════════════════════════════
    {"name": "words that often go together collocations",
     "prompt": "Generate facts about words that commonly appear together in English. Examples: 'We say make a decision, not do a decision.' 'We say heavy rain, not strong rain.' 'We say fast food, not quick food.' 'We say take a photo, not make a photo.' 'We say strong coffee, not powerful coffee.' 'We say pay attention, not give attention.'"},
    {"name": "words with multiple meanings",
     "prompt": "Generate facts about common English words that have multiple meanings. Examples: 'The word bank can mean a financial institution or the side of a river.' 'The word light can mean not heavy or it can mean brightness.' 'The word run can mean to move quickly or to operate a machine.' 'The word fair can mean just and equal or it can mean a carnival or festival.' 'The word spring can mean the season, a coil of metal, or a water source.'"},
    {"name": "transition words and what they signal",
     "prompt": "Generate facts about transition words and what kind of connection they signal. Examples: 'The word however signals a contrast or contradiction is coming.' 'The word therefore signals a conclusion or result.' 'The word furthermore signals additional supporting information.' 'The word meanwhile signals something happening at the same time.' 'The word instead signals a replacement or alternative.'"},
    {"name": "intensity scales of words",
     "prompt": "Generate facts about how words form intensity scales. Examples: 'From least to most: warm, hot, boiling.' 'From least to most: dislike, hate, despise.' 'From least to most: happy, joyful, ecstatic.' 'From least to most: walk, jog, run, sprint.' 'From least to most: cool, cold, freezing.' 'From smallest to largest: tiny, small, medium, large, huge, enormous.'"},
]


# ─── Custom Ollama prompt that generates FOUNDATIONAL knowledge ─────────

FOUNDATION_SYSTEM = """You are a knowledge formatter for an AI training system.
You output ONLY valid JSON — no markdown, no explanation, no code blocks.
Your JSON must have this exact structure:
{"items": [{"content": "fact", "associations": ["related fact"]}]}

IMPORTANT RULES:
- Each "content" must be a COMPLETE, standalone sentence
- Write in simple, clear English
- Each fact should be self-contained (understandable without context)
- Associations should be genuinely related facts, not just rewordings
- Focus on RELATIONSHIPS, CAUSES, and MEANINGS — not just raw facts
- Avoid sentence fragments or dictionary-style entries"""


def generate_foundation_knowledge(topic_name, prompt_hint, count=10, model=DEFAULT_MODEL):
    """Generate knowledge using the custom prompt hint for each topic."""
    import urllib.request
    import urllib.error
    import json

    prompt = f"""Generate exactly {count} pieces of knowledge about: {topic_name}

STYLE GUIDE: {prompt_hint}

Output ONLY this JSON structure:
{{
  "items": [
    {{"content": "A complete sentence expressing one clear idea", "associations": ["A related fact that connects to this"]}},
    {{"content": "Another distinct piece of knowledge", "associations": ["A connecting fact"]}}
  ]
}}

Rules:
- Each "content" must be one clear, complete sentence (10-25 words)
- Each "associations" array has exactly 1 related fact
- Every fact must be DISTINCT — no repetition
- Use simple, precise language
- Output ONLY JSON, nothing else"""

    payload = {
        "model": model,
        "prompt": prompt,
        "system": FOUNDATION_SYSTEM,
        "stream": False,
        "format": "json",
        "options": {"temperature": 0.4, "top_p": 0.9}
    }

    data = json.dumps(payload).encode("utf-8")
    req = urllib.request.Request(
        "http://localhost:11434/api/generate",
        data=data,
        headers={"Content-Type": "application/json"}
    )
    try:
        with urllib.request.urlopen(req, timeout=180) as resp:
            result = json.loads(resp.read().decode("utf-8"))
            text = result.get("response", "")
            if not text:
                return None
            import re
            text = text.strip()
            if "```" in text:
                text = re.sub(r'```(?:json)?\n?', '', text).strip()
            text = re.sub(r',\s*([}\]])', r'\1', text)
            try:
                parsed = json.loads(text)
                items = parsed.get("items", [])
                valid = [i for i in items if isinstance(i, dict) and "content" in i]
                return valid if valid else None
            except json.JSONDecodeError:
                match = re.search(r'\{[\s\S]*\}', text)
                if match:
                    try:
                        parsed = json.loads(match.group())
                        items = parsed.get("items", [])
                        valid = [i for i in items if isinstance(i, dict) and "content" in i]
                        return valid if valid else None
                    except json.JSONDecodeError:
                        pass
                print(f"[ollama] Could not parse: {text[:200]}")
                return None
    except urllib.error.URLError as e:
        print(f"[ollama] Error: {e}")
        print("[ollama] Is Ollama running? Start with: ollama serve")
        return None


def kill_existing_mcp():
    """Kill any running lu_ai_mcp.exe processes."""
    try:
        result = subprocess.run(
            ["taskkill", "/F", "/IM", "lu_ai_mcp.exe"],
            capture_output=True, text=True
        )
        if result.returncode == 0:
            print("[batch] Killed existing MCP process(es)")
            time.sleep(1)
    except Exception:
        pass


def check_ollama():
    """Verify Ollama is running."""
    import urllib.request
    try:
        urllib.request.urlopen("http://localhost:11434/api/tags", timeout=5)
        return True
    except Exception:
        return False


def main():
    parser = argparse.ArgumentParser(description="Foundation Training")
    parser.add_argument("--start-from", type=int, default=0,
                        help="Resume from topic index")
    parser.add_argument("--dry-run", action="store_true",
                        help="List topics only")
    parser.add_argument("--model", "-m", default=DEFAULT_MODEL,
                        help="Ollama model")
    parser.add_argument("--count", "-c", type=int, default=10,
                        help="Facts per topic")
    args = parser.parse_args()

    print(f"Lu Ai Foundation Training")
    print(f"Topics: {len(TOPICS)}")
    print(f"Facts per topic: {args.count}")
    print(f"Model: {args.model}")
    print(f"Start from: #{args.start_from}")
    print()

    if args.dry_run:
        for i, t in enumerate(TOPICS):
            marker = ">>>" if i == args.start_from else "   "
            print(f"{marker} [{i:3d}] {t['name']}")
        print(f"\nTotal: {len(TOPICS)} topics")
        return

    if not check_ollama():
        print("ERROR: Ollama is not running!")
        print("Start it with: ollama serve")
        sys.exit(1)

    kill_existing_mcp()

    bridge = MCPBridge()
    bridge.start()

    stats = bridge._get_stats()
    start_nodes = stats.get("active_nodes", 0) if stats else 0
    print(f"\nStarting at {start_nodes} nodes.")
    print(f"{'='*50}\n")

    trained = 0
    failed = 0
    start_time = time.time()

    for i, topic in enumerate(TOPICS):
        if i < args.start_from:
            continue

        current_stats = bridge._get_stats()
        current_nodes = current_stats.get("active_nodes", 0) if current_stats else 0

        print(f"[{i+1}/{len(TOPICS)}] {topic['name']} (brain: {current_nodes} nodes)")

        items = generate_foundation_knowledge(
            topic["name"], topic["prompt"], args.count, args.model
        )
        if not items:
            print(f"  FAILED — skipping")
            failed += 1
            continue

        print(f"  [ollama] Got {len(items)} items")
        result = bridge.batch_train(items)

        if result and "total_learned" in result:
            print(f"  +{result['total_learned']} nodes, +{result.get('total_bonds', 0)} bonds")
        elif result:
            print(f"  Result: {str(result)[:100]}")
        else:
            print(f"  No result from batch_train")

        trained += 1

        # Save every 10 topics
        if trained % 10 == 0:
            bridge.save()
            elapsed = time.time() - start_time
            cs = bridge._get_stats()
            cn = cs.get("active_nodes", 0) if cs else 0
            print(f"  [save] {cn} nodes | {elapsed:.0f}s | {trained} topics\n")

    bridge.save()
    elapsed = time.time() - start_time

    final_stats = bridge._get_stats()
    final_nodes = final_stats.get("active_nodes", 0) if final_stats else 0

    print(f"\n{'='*50}")
    print(f"Foundation training complete!")
    print(f"Time: {elapsed:.0f}s ({elapsed/60:.1f} min)")
    print(f"Topics: {trained} trained, {failed} failed")
    print(f"Nodes: {start_nodes} -> {final_nodes} (+{final_nodes - start_nodes})")
    print(f"{'='*50}")

    bridge.stop()


if __name__ == "__main__":
    main()
