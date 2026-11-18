# Intent

Assets:
- Player: 8x16 15-bit color (4bpp)
    - animations: 4 frames each (may use temp/duplicates until later)
    idle, walk, run, shoot bow, swing blade, take damage, eat, start fire
    facing front, facing right, facing back
    - actions: 
        blade = Button A
        bow = Hold A
        weapon select = Button Select
        run = Hold L
        eat/light fire/grab cooked food = Button R
        pause = Button Start
    - attributes:
        hunger = 3

- Etc:
    - Bow: within animation(s)
    - Blade: within animation(s)
    - Fire: 8x8 15-bit color (4bpp) 
        - states: lit / unlit
        - attributes:
            can hold only 1 meat at a time
            meat takes about 10 seconds to cook

- World: 8x8 Tiled 15-bit color (4bpp)
    - size: 1024x1024 tiles with border
    - biome: simple forest and meadows
    - rocks/trees: collision boxes on base

- Music: XM Music
    title, pause, game over, playing

- Sound: WAV or other format(s)
    - Player: walk, run, hurt, eat, start fire, shoot bow, swing blade
    - Capis: walk, run, hurt
    - World: wind, leave rustle, birds/owls

- Capis: 8x8 15-bit color (4bpp)
    - animations: 4 frames each (may use temp/duplicates until later)
    idle, walk, run, hurt
    - attributes:
        health = 4 hits
        drops = 1 meat

Gameplay:
- Main:
    Player must hunt Capis
    Player must cook meat
    Player must eat meat

Project:
- Docs: you MUST document everything with comments. 
- Checks: run a cargo check / build to verify the game complies cleanly. 
- Directory: use separate files for game states
    title, playing, game over, credits, and save select
- Title:
    - attributes:
        new start, load save, options, credits
- Credits:
    - attributes:
        scroll, exit to title
- Playing:
    - attributes:
        pause: allows save, options, exit
            options:
                music, sound
        playing: standard running
- Game Over:
    - attributes:
        overall game stats, credits, exit
