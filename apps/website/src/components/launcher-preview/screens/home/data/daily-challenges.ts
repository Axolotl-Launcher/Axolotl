export type ChallengeDifficulty = 'easy' | 'medium' | 'hard'

export type DailyChallenge = {
    id: string
    difficulty: ChallengeDifficulty
    text: {
        'en-US': string
        'zh-CN': string
        'zh-TW': string
    }
}

export const dailyChallenges: readonly DailyChallenge[] = [
    {
        id: 'stone-witch',
        difficulty: 'hard',
        text: {
            'en-US': 'Defeat a witch using only stone tools.',
            'zh-CN': '只用石制工具击败一只女巫。',
            'zh-TW': '只用石製工具擊敗一隻女巫。',
        },
    },
    {
        id: 'no-torch-cave',
        difficulty: 'medium',
        text: {
            'en-US': 'Fully explore a cave without placing a single torch.',
            'zh-CN': '在不放置任何火把的情况下探索完一个洞穴。',
            'zh-TW': '在不放置任何火把的情況下探索完一個洞穴。',
        },
    },
    {
        id: 'flower-garden',
        difficulty: 'easy',
        text: {
            'en-US': 'Plant a garden with at least eight different kinds of flowers.',
            'zh-CN': '种一座至少包含八种不同花朵的花园。',
            'zh-TW': '種一座至少包含八種不同花朵的花園。',
        },
    },
    {
        id: 'trader-llama',
        difficulty: 'easy',
        text: {
            'en-US': 'Dye a carpet for a trader llama in your favorite color.',
            'zh-CN': '给一只行商羊驼装上你最喜欢颜色的地毯。',
            'zh-TW': '給一隻商駝裝上你最喜歡顏色的地毯。',
        },
    },
    {
        id: 'boat-river',
        difficulty: 'easy',
        text: {
            'en-US': 'Follow a river by boat until it reaches the ocean.',
            'zh-CN': '乘船沿一条河流一直航行到大海。',
            'zh-TW': '乘船沿一條河流一直航行到大海。',
        },
    },
    {
        id: 'sky-bridge',
        difficulty: 'medium',
        text: {
            'en-US': 'Build a bridge between two mountain peaks.',
            'zh-CN': '在两座山峰之间架起一座桥。',
            'zh-TW': '在兩座山峰之間架起一座橋。',
        },
    },
    {
        id: 'wheat-hundred',
        difficulty: 'medium',
        text: {
            'en-US': 'Harvest one hundred wheat in a single day.',
            'zh-CN': '在游戏内的一天里收获一百株小麦。',
            'zh-TW': '在遊戲內的一天裡收穫一百株小麥。',
        },
    },
    {
        id: 'pet-parade',
        difficulty: 'medium',
        text: {
            'en-US': 'Tame a wolf, a cat, and a parrot in the same world.',
            'zh-CN': '在同一个世界里驯服狼、猫和鹦鹉各一只。',
            'zh-TW': '在同一個世界裡馴服狼、貓和鸚鵡各一隻。',
        },
    },
    {
        id: 'nether-photo',
        difficulty: 'medium',
        text: {
            'en-US': 'Take a screenshot standing on top of a bastion.',
            'zh-CN': '站在一座堡垒遗迹顶端截一张图。',
            'zh-TW': '站在一座堡壘遺蹟頂端截一張圖。',
        },
    },
    {
        id: 'village-lights',
        difficulty: 'easy',
        text: {
            'en-US': 'Light up every street of a village with lanterns.',
            'zh-CN': '用灯笼点亮一座村庄的每一条街道。',
            'zh-TW': '用燈籠點亮一座村莊的每一條街道。',
        },
    },
    {
        id: 'iron-armor-day',
        difficulty: 'medium',
        text: {
            'en-US': 'Craft a full set of iron armor before the first night falls.',
            'zh-CN': '在第一个夜晚降临前做出整套铁甲。',
            'zh-TW': '在第一個夜晚降臨前做出整套鐵甲。',
        },
    },
    {
        id: 'fishing-rain',
        difficulty: 'easy',
        text: {
            'en-US': 'Go fishing in the rain until you catch a treasure item.',
            'zh-CN': '在雨中钓鱼，直到钓上一件宝藏物品。',
            'zh-TW': '在雨中釣魚，直到釣上一件寶藏物品。',
        },
    },
    {
        id: 'map-wall',
        difficulty: 'hard',
        text: {
            'en-US': 'Complete a 2x2 map wall of the land around your base.',
            'zh-CN': '为基地周边地区制作一面 2×2 的地图墙。',
            'zh-TW': '為基地周邊地區製作一面 2×2 的地圖牆。',
        },
    },
    {
        id: 'no-bed-three-days',
        difficulty: 'medium',
        text: {
            'en-US': 'Survive three in-game days without sleeping.',
            'zh-CN': '连续三个游戏日不睡觉并活下来。',
            'zh-TW': '連續三個遊戲日不睡覺並活下來。',
        },
    },
    {
        id: 'melon-stand',
        difficulty: 'easy',
        text: {
            'en-US': 'Open a little roadside melon stand near a village.',
            'zh-CN': '在村庄旁开一个小小的西瓜路边摊。',
            'zh-TW': '在村莊旁開一個小小的西瓜路邊攤。',
        },
    },
    {
        id: 'horse-race',
        difficulty: 'easy',
        text: {
            'en-US': 'Build a racing track and time yourself on horseback.',
            'zh-CN': '修一条赛道，骑马给自己计一次时。',
            'zh-TW': '建一條賽道，騎馬給自己計一次時。',
        },
    },
    {
        id: 'ocean-monument-peek',
        difficulty: 'hard',
        text: {
            'en-US': 'Swim a full lap around an ocean monument without potions.',
            'zh-CN': '不喝药水绕海底神殿游完整整一圈。',
            'zh-TW': '不喝藥水繞海底遺蹟游完整整一圈。',
        },
    },
    {
        id: 'redstone-door',
        difficulty: 'medium',
        text: {
            'en-US': 'Build a hidden redstone door for your base entrance.',
            'zh-CN': '为基地入口做一扇隐藏的红石门。',
            'zh-TW': '為基地入口做一扇隱藏的紅石門。',
        },
    },
    {
        id: 'sheep-rainbow',
        difficulty: 'medium',
        text: {
            'en-US': 'Collect wool in every color of the rainbow from your own sheep.',
            'zh-CN': '从自己养的羊身上集齐彩虹七色的羊毛。',
            'zh-TW': '從自己養的羊身上集齊彩虹七色的羊毛。',
        },
    },
    {
        id: 'tree-house',
        difficulty: 'medium',
        text: {
            'en-US': 'Move into a treehouse for one full in-game day.',
            'zh-CN': '搬进树屋，度过完整的一个游戏日。',
            'zh-TW': '搬進樹屋，度過完整的一個遊戲日。',
        },
    },
    {
        id: 'zero-craft-night',
        difficulty: 'hard',
        text: {
            'en-US': 'Survive a night in the open without crafting anything.',
            'zh-CN': '不合成任何物品，在野外撑过一整夜。',
            'zh-TW': '不合成任何物品，在野外撐過一整夜。',
        },
    },
    {
        id: 'librarian-friend',
        difficulty: 'easy',
        text: {
            'en-US': 'Trade with a librarian until you unlock every trade.',
            'zh-CN': '和一名图书管理员交易，直到解锁其全部交易项。',
            'zh-TW': '和一名圖書管理員交易，直到解鎖其全部交易項。',
        },
    },
    {
        id: 'snow-golem-army',
        difficulty: 'easy',
        text: {
            'en-US': 'Build five snow golems and lead them on a patrol.',
            'zh-CN': '做五个雪傀儡，带着它们巡逻一圈。',
            'zh-TW': '做五個雪人，帶著它們巡邏一圈。',
        },
    },
    {
        id: 'mineshaft-exit',
        difficulty: 'medium',
        text: {
            'en-US': 'Enter an abandoned mineshaft and find a different way out.',
            'zh-CN': '进入一座废弃矿井，并从另一条路走出来。',
            'zh-TW': '進入一座廢棄礦坑，並從另一條路走出來。',
        },
    },
    {
        id: 'desert-well',
        difficulty: 'easy',
        text: {
            'en-US': 'Find a desert well and build a tiny oasis around it.',
            'zh-CN': '找到一口沙漠水井，围着它造一个小绿洲。',
            'zh-TW': '找到一口沙漠水井，圍著它造一個小綠洲。',
        },
    },
    {
        id: 'creeper-dodge',
        difficulty: 'medium',
        text: {
            'en-US': 'Let a creeper chase you through a village without any explosion.',
            'zh-CN': '让一只苦力怕追着你穿过村庄，全程不发生爆炸。',
            'zh-TW': '讓一隻苦力怕追著你穿過村莊，全程不發生爆炸。',
        },
    },
    {
        id: 'bee-keeper',
        difficulty: 'easy',
        text: {
            'en-US': 'Set up a bee farm and bottle your first honey.',
            'zh-CN': '建一个蜜蜂农场，装出第一瓶蜂蜜。',
            'zh-TW': '建一個蜜蜂農場，裝出第一瓶蜂蜜。',
        },
    },
    {
        id: 'lava-bucket-nerves',
        difficulty: 'hard',
        text: {
            'en-US': 'Cross a lava lake using only buckets of water.',
            'zh-CN': '只用几桶水渡过一片熔岩湖。',
            'zh-TW': '只用幾桶水渡過一片熔岩湖。',
        },
    },
    {
        id: 'pillager-tower',
        difficulty: 'medium',
        text: {
            'en-US': 'Take over a pillager outpost and fly your own banner on top.',
            'zh-CN': '占领一座掠夺者前哨站，在顶上插上你自己的旗帜。',
            'zh-TW': '占領一座掠奪者前哨站，在頂上插上你自己的旗幟。',
        },
    },
    {
        id: 'mushroom-house',
        difficulty: 'easy',
        text: {
            'en-US': 'Build a cottage inside a giant mushroom.',
            'zh-CN': '在一朵巨型蘑菇里建一座小屋。',
            'zh-TW': '在一朵巨型蘑菇裡建一座小屋。',
        },
    },
    {
        id: 'skeleton-duel',
        difficulty: 'medium',
        text: {
            'en-US': 'Win a bow duel against a skeleton without moving your feet.',
            'zh-CN': '站在原地不移动，用弓赢下与骷髅的对射。',
            'zh-TW': '站在原地不移動，用弓贏下與骷髏的對射。',
        },
    },
    {
        id: 'chest-organizer',
        difficulty: 'easy',
        text: {
            'en-US': 'Sort your entire storage room and label every chest.',
            'zh-CN': '整理你的整个仓库，并给每个箱子贴上标签。',
            'zh-TW': '整理你的整個倉庫，並給每個箱子貼上標籤。',
        },
    },
    {
        id: 'nether-tunnel',
        difficulty: 'hard',
        text: {
            'en-US': 'Dig a fully lit tunnel connecting two nether portals.',
            'zh-CN': '挖一条照明齐全的通道，连接两座下界传送门。',
            'zh-TW': '挖一條照明齊全的通道，連接兩座地獄傳送門。',
        },
    },
    {
        id: 'jungle-temple',
        difficulty: 'medium',
        text: {
            'en-US': 'Loot a jungle temple without triggering a single trap.',
            'zh-CN': '不触发任何机关，搜刮完一座丛林神庙。',
            'zh-TW': '不觸發任何機關，搜刮完一座叢林遺蹟。',
        },
    },
    {
        id: 'ice-boat-highway',
        difficulty: 'medium',
        text: {
            'en-US': 'Build a short ice road and set your personal boat speed record.',
            'zh-CN': '修一小段冰道，刷新你自己的船速纪录。',
            'zh-TW': '建一小段冰道，刷新你自己的船速紀錄。',
        },
    },
    {
        id: 'armor-stand-gallery',
        difficulty: 'easy',
        text: {
            'en-US': 'Display your old gear in an armor stand gallery.',
            'zh-CN': '用盔甲架办一个展览，陈列你的旧装备。',
            'zh-TW': '用盔甲架辦一個展覽，陳列你的舊裝備。',
        },
    },
    {
        id: 'day-one-diamond',
        difficulty: 'hard',
        text: {
            'en-US': 'Find a diamond before the end of the first in-game day.',
            'zh-CN': '在第一个游戏日结束前找到一颗钻石。',
            'zh-TW': '在第一個遊戲日結束前找到一顆鑽石。',
        },
    },
    {
        id: 'shipwreck-restore',
        difficulty: 'medium',
        text: {
            'en-US': 'Find a shipwreck and restore it to a seaworthy ship.',
            'zh-CN': '找到一艘沉船，把它修复成能"出海"的样子。',
            'zh-TW': '找到一艘沉船，把它修復成能「出海」的樣子。',
        },
    },
    {
        id: 'campfire-cookout',
        difficulty: 'easy',
        text: {
            'en-US': 'Cook a full dinner for yourself on campfires only.',
            'zh-CN': '只用营火为自己做一顿丰盛的晚餐。',
            'zh-TW': '只用營火為自己做一頓豐盛的晚餐。',
        },
    },
    {
        id: 'lush-cave-picnic',
        difficulty: 'easy',
        text: {
            'en-US': 'Have a picnic in a lush cave surrounded by glow berries.',
            'zh-CN': '在发光浆果环绕的繁茂洞穴里野餐一次。',
            'zh-TW': '在螢光莓環繞的繁茂洞穴裡野餐一次。',
        },
    },
]
