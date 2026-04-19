//! Demo entries for the TTS test page.
//!
//! 29 demo entries from `assets/demo.jsonl` in the MOSS-TTS-Nano
//! reference repo. Each entry has a display name, role (voice),
//! and demo text.

/// A single demo entry from `demo.jsonl`.
#[derive(Debug, Clone)]
pub struct DemoEntry {
    /// Display name with emoji flag.
    pub name: String,
    /// Voice role (e.g. "assets/audio/zh_1.wav").
    pub role: String,
    /// Demo text to synthesize.
    pub text: String,
    /// Resolved voice name (derived from role).
    pub voice_name: String,
}

/// All 29 demo entries from `assets/demo.jsonl`.
pub fn all_demos() -> Vec<DemoEntry> {
    vec![
        DemoEntry {
            name: "🇨🇳 欢迎关注模思智能".to_string(),
            role: "assets/audio/zh_1.wav".to_string(),
            text: "欢迎关注模思智能、上海创智学院与复旦大学自然语言处理实验室。".to_string(),
            voice_name: "Junhao".to_string(),
        },
        DemoEntry {
            name: "🇨🇳 深夜温柔晚安".to_string(),
            role: "assets/audio/zh_6.wav".to_string(),
            text: "夜深了，城市的灯一盏一盏慢慢安静下来，你也终于有了一点属于自己的时间。或许今天并不轻松，可能有些事情没有按照期待发展，也可能只是单纯地感到疲惫。但没关系，现在这一刻，你不需要证明什么，也不需要赶着去完成什么，只需要好好待在这里，和自己待一会儿。

想象一下，窗外有一点点微风，轻轻地吹动树叶，发出很细很柔的声音。这样的声音，不急不躁，就像时间本来的样子。你可以慢慢地呼吸，让空气一点点进入身体，再轻轻地呼出来。所有紧绷的情绪，都可以随着呼气一点点放松下来。

有时候我们会对自己太严格，总觉得应该更好一点、更快一点、更完美一点。但其实，你已经走了很远的路。那些你以为不起眼的坚持，那些别人看不到的努力，都在一点一点地塑造现在的你。你不需要一直强大，也可以偶尔脆弱，偶尔停下来。停下来，并不是失败，而是给自己一点空间，好让内心重新变得柔软。

如果你愿意，可以把今天发生的一件小事轻轻地想一想，也许是一杯温热的水，一句简单的问候，或者是某个不经意的微笑。这些细碎的温暖，像夜空里不太显眼的星星，但它们一直都在，默默地陪着你。

现在，就这样慢慢地，把肩膀放松下来，让呼吸变得更轻一点。你已经做得很好了，真的。接下来的时间，不需要焦虑未来，也不需要回头纠结过去。就只是这一刻，你是安全的，是被允许安静下来的。晚安，愿你在柔软的梦里，被温柔地接住。".to_string(),
            voice_name: "Lingyu".to_string(),
        },
        DemoEntry {
            name: "🇨🇳 台湾腔".to_string(),
            role: "assets/audio/zh_4.wav".to_string(),
            text: "唉你知道吗，今天真的有够热欸，我一出门就觉得整个人快融化了啦。

然后我刚刚去买饮料，结果前面排超多人，我就在那边等超久，真的有点小崩溃欸。

不过还好最后买到我最爱的珍奶，心情就有比较好一点这样。".to_string(),
            voice_name: "Yuewen".to_string(),
        },
        DemoEntry {
            name: "🇨🇳 京味胡同闲聊".to_string(),
            role: "assets/audio/zh_3.wav".to_string(),
            text: "您说这天儿啊，这两天是真不错，风也不大，太阳一出来吧，人心里头都跟着亮堂了不少。早上起来溜达一圈儿，公园里那大爷大妈都开始活动开了，有打太极的，有甩胳膊的，还有边走边聊的，甭提多热闹了。

我跟您说啊，这日子吧，其实也没那么复杂。您甭老琢磨那些个烦心事儿，该吃吃，该喝喝，心里头敞亮点儿，比什么都强。您要是老跟自个儿较劲，那多累啊，是不是这理儿？

再说这胡同里头，人情味儿那是没得说。谁家有点啥事儿，招呼一声，邻里街坊都能搭把手。早上买个豆浆油条，跟老板寒暄两句，这一天的心情都不一样了。您瞧，这不就是咱老北京最地道的那点儿味儿嘛。

所以啊，甭着急，慢慢来。日子一天天过，您踏踏实实的，比啥都靠谱。".to_string(),
            voice_name: "Xiaoyu".to_string(),
        },
        DemoEntry {
            name: "🇨🇳 中国人的时间观念与文化逻辑".to_string(),
            role: "assets/audio/zh_10.wav".to_string(),
            text: "各位观众朋友，大家好。
今天这一讲，我们从一个几乎每个人都绕不开的话题谈起——中国人的时间观念。

如果你细心观察就会发现，中国文化中，对“时间”的理解，与许多西方社会并不完全相同。我们很少单纯地把时间看成一条不断向前流逝的直线，而更倾向于把它理解为一种循环往复、层层递进的过程。

这种观念，最早可以追溯到农业文明。春生、夏长、秋收、冬藏，年复一年，周而复始。时间不是被“消耗”的，而是被“等待”和“积累”的。正因为如此，中国人在面对变化时，往往更强调“时机”二字，而不是速度本身。

你会发现，在《周易》中，最重要的不是吉凶判断，而是“时”。同样一件事，做得早了，叫冒进；做得晚了，叫迟误；唯有在合适的时间行动，才能事半功倍。这种思想，深刻影响了后世中国人的处世方式。

再往后看，《史记》中对人物的评价，也常常不只看成败，而要放到时代背景中去衡量。一个人是否成功，并不完全取决于个人能力，还取决于他是否“生逢其时”。

这也解释了为什么中国文化中，对“急功近利”始终保持警惕。古人常说：“欲速则不达。”并不是反对进取，而是提醒我们，脱离时间规律的努力，往往适得其反。

因此，中国人的智慧，往往体现为一种耐心。不是停滞不前，而是在等待中积蓄力量，在沉默中观察变化。

当我们重新理解了这种时间观，也许就能更好地理解中国文化那种看似缓慢，却极其坚韧的生命力。".to_string(),
            voice_name: "Adam".to_string(),
        },
        DemoEntry {
            name: "🇨🇳 杨幂 - 与自己同行".to_string(),
            role: "assets/audio/zh_11.wav".to_string(),
            text: "有些人喜欢被照顾，
而我更习惯照亮自己。

不是不需要依靠，
只是明白——
真正能陪你走到最后的，
从来都不是运气。

我见过凌晨四点的城市，
也见过掌声散去后的安静。
那些看起来毫不费力的从容，
其实都藏着一次次咬牙坚持。

你可以说我清醒，
也可以说我冷静。
但只有我自己知道，
每一次选择背后，
我都为自己负责。

世界很吵，
声音太多了。
有人教你妥协，
有人劝你低头，
也有人告诉你——
“算了吧，别太认真。”

可偏偏是认真，
让我走到今天。

我喜欢掌控节奏，
喜欢在风来的时候站稳，
也喜欢在夜深人静时，
对自己说一句：
“今天，做得不错。”

我不急着证明什么，
时间会替我说话。
我不害怕孤独，
因为独处让我更清楚——
我想要什么，
我值得什么。

如果你也正在路上，
别急。
慢一点没关系，
走稳一点就好。

你不需要成为别人期待的样子，
你只需要成为——
那个连自己都欣赏的人。

这一路，
我会继续走下去。
不张扬，
不退场，
刚刚好地，
发着光。".to_string(),
            voice_name: "Adam".to_string(),
        },
        DemoEntry {
            name: "🇺🇸 Welcome to OpenMOSS".to_string(),
            role: "assets/audio/en_6.wav".to_string(),
            text: "OpenMOSS Team is jointly established by Shanghai Innovation Institution, Fudan University NLP Lab, and MOSI Intelligence, exploring an innovative development model centered on deep integration of industry, academia, and research.".to_string(),
            voice_name: "Nathan".to_string(),
        },
        DemoEntry {
            name: "🇺🇸 The Bitter Lesson".to_string(),
            role: "assets/audio/en_2.wav".to_string(),
            text: "The biggest lesson that can be read from 70 years of AI research is that general methods that leverage computation are ultimately the most effective, and by a large margin. The ultimate reason for this is Moore's law, or rather its generalization of continued exponentially falling cost per unit of computation.

Most AI research has been conducted as if the computation available to the agent were constant, in which case leveraging human knowledge would be one of the only ways to improve performance. But over a slightly longer time than a typical research project, massively more computation inevitably becomes available.

Seeking an improvement that makes a difference in the shorter term, researchers seek to leverage their human knowledge of the domain, but the only thing that matters in the long run is the leveraging of computation. These two need not run counter to each other, but in practice they tend to. Time spent on one is time not spent on the other.

There are psychological commitments to investment in one approach or the other. And the human-knowledge approach tends to complicate methods in ways that make them less suited to taking advantage of general methods leveraging computation. There were many examples of AI researchers' belated learning of this bitter lesson, and it is instructive to review some of the most prominent.".to_string(),
            voice_name: "Ava".to_string(),
        },
        DemoEntry {
            name: "🇺🇸 English News".to_string(),
            role: "assets/audio/en_4.wav".to_string(),
            text: "Good evening, and welcome to the news.

Today’s top stories: Global economic activity continues to show steady growth, with several countries reporting improvements in technology and green energy development. Experts say these trends are expected to support long-term sustainability and innovation.

Meanwhile, cities around the world are taking new steps to improve urban living conditions, focusing on environmental protection and public services. These efforts aim to enhance the quality of life for residents.

In other news, cultural and tourism activities are seeing a strong recovery, attracting visitors both locally and internationally. Analysts believe this will further boost regional economies in the coming months.

That’s all for today’s broadcast.

Thank you for watching, and have a great evening.".to_string(),
            voice_name: "Adam".to_string(),
        },
        DemoEntry {
            name: "🇺🇸 A Gentle Reminder".to_string(),
            role: "assets/audio/en_3.wav".to_string(),
            text: "It’s okay to feel a little tired or unsure sometimes. Not everything needs to be clear right away. Take things slowly, one step at a time. Even the smallest step forward is still progress.

If today feels heavy, just allow yourself to rest. It doesn’t mean you’re falling behind. Things will become easier, little by little. And you don’t have to go through it all at once.

You’re doing better than you think.".to_string(),
            voice_name: "Bella".to_string(),
        },
        DemoEntry {
            name: "🇺🇸 Taylor Swift - You Are Not Alone Tonight".to_string(),
            role: "assets/audio/en_7.wav".to_string(),
            text: "Tonight, I just want to take a second and breathe this in with you.
Because moments like this don’t happen by accident. They’re built—one lyric at a time, one late night at a time, one brave decision at a time. They’re built by people who keep showing up, even when life is loud, even when the world is heavy, even when they’re not sure anyone sees the effort they’re making.
And if you’re here tonight, I want you to hear me say this clearly: I see you.
I’ve learned something over the years—about love, and loss, and hope, and the way we keep becoming ourselves. It’s that you can be soft and still be strong. You can be kind and still have boundaries. You can miss someone and still move forward. You can have a heart that’s been cracked open and still let it beat for something beautiful again.
So if you walked in here carrying anything—regret, grief, pressure, insecurity, the feeling that you’re behind, the feeling that you’re too much, or not enough—I want you to know you don’t have to hold it alone tonight. You get to set it down for a while. You get to be human here. You get to sing it out, shout it out, dance it out, cry it out, laugh it out—whatever you need.
And maybe the bravest thing you’ve done all week is simply making it to this moment. Maybe you’re in the middle of a beginning that nobody can see yet. Maybe you’re quietly rebuilding. Maybe you’re choosing yourself after a long time of choosing what everyone else wanted. Maybe you’re learning to forgive someone—or learning to forgive yourself.
That kind of growth is not always pretty. Sometimes it’s messy. Sometimes it’s lonely. Sometimes it looks like taking a step back so you can take two steps forward. But it’s real. And it matters.
I also want to say: your story is not defined by the worst thing you’ve been through, or the harshest thing someone’s said about you, or the version of you that was trying to survive. You’re allowed to evolve. You’re allowed to outgrow. You’re allowed to rewrite the narrative, even if somebody else thought they had the pen.
And if nobody has told you lately—there is nothing wrong with wanting more. More joy. More peace. More honesty. More love that feels safe. More friends who celebrate you. More mornings where your chest feels lighter. You’re not asking for too much. You’re just finally asking for what you deserve.
So thank you—for listening, for caring, for showing up with your whole heart. Thank you for making this feel like a home we only get to build together. Tonight, let’s be fearless about feeling everything. Let’s be gentle with ourselves. Let’s be loud about the things that bring us alive.
And when you leave here, I hope you take one thing with you: you are not alone. Not in your dreams, not in your doubts, not in your healing, not in your happiness.".to_string(),
            voice_name: "Nathan".to_string(),
        },
        DemoEntry {
            name: "🇺🇸 The Quiet Motion of the World".to_string(),
            role: "assets/audio/en_8.wav".to_string(),
            text: "In the quiet hours before dawn, the world looks unfinished.
Streets are empty, windows are dark, and the air holds its breath as if waiting for a cue.
But beneath the stillness, everything is moving.
Water is traveling through pipes.
Electricity is humming along invisible lines.
Seeds are pushing against soil.
Somewhere, a hand reaches for a switch, and a day begins.

We live inside systems so familiar we forget they are miracles.
Food arrives as if by habit.
Messages cross oceans in an instant.
A storm forms far away, and we feel its consequences before we ever see its clouds.
There is a rhythm to modern life, a steady pulse of choices and routines, and yet the smallest change can send ripples through everything—one missed train, one broken bridge, one unexpected word.

If you listen closely, you can hear the planet speaking in many languages.
The creak of old buildings settling into the ground.
The soft friction of tires on wet pavement.
The distant laughter that escapes from a doorway before it closes again.
Even silence has texture: a pause between waves, a moment between decisions, the space where something new can enter.

Time, too, leaves fingerprints.
It gathers in photographs, in worn corners of steps, in the way hands remember a task long after the mind has forgotten learning it.
It lives in the stories we tell to explain who we are, and in the stories we never tell, because we don’t yet have words for them.
Every place carries layers—what was built, what was erased, what endured.
And in those layers, the past isn’t behind us.
It is under us, around us, and sometimes within us.

Across the world, people wake with different urgencies.
Some chase the light, some hide from it.
Some measure the day in deliveries, in harvests, in shifts and deadlines.
Others measure it in recovery, in waiting rooms, in long walks taken just to keep going.
But everyone, in some way, is trying to make a life out of ordinary hours—trying to turn uncertainty into something they can hold.

There is a temptation to believe that history is shaped only by grand events.
But most change begins quietly.
A decision made at a kitchen table.
A habit broken.
A promise kept when no one is watching.
A question asked for the first time.
These are the small forces that accumulate, like rain carving stone, like footsteps wearing a path.

And when the sun rises, it doesn’t announce a new world.
It reveals the one that was already there—complicated, connected, fragile, stubbornly alive.
The day arrives carrying both inheritance and possibility.
The same roads, the same skies, the same worries.
And yet, somewhere in the familiar, something has shifted.

Because the story of our time is not only about what we build, or what we lose.
It is about what we notice.
It is about the courage to look closely, to listen longer, to remember that every moment is part of a larger motion.
And if we can see that motion—if we can understand how each life touches another—then the world, unfinished as it is, becomes a place where meaning can still be made.".to_string(),
            voice_name: "Nathan".to_string(),
        },
        DemoEntry {
            name: "🇯🇵 ニュース".to_string(),
            role: "assets/audio/jp_2.wav".to_string(),
            text: "こんばんは、ニュースです。本日、技術革新の分野で重要な進展があり、経済成長への期待が高まっています。国際的には、各国が協力関係を強化し、貿易や文化交流の拡大を図っています。国内では、公共サービスの改善が進められ、人々の生活の質向上が目指されています。以上、主なニュースでした。".to_string(),
            voice_name: "Yui".to_string(),
        },
        DemoEntry {
            name: "🇰🇷 뉴스".to_string(),
            role: "assets/audio/en_4.wav".to_string(),
            text: "여러분 안녕하십니까, 뉴스입니다. 오늘 기술 혁신 분야에서 중요한 성과가 이루어져 경제 성장에 대한 기대가 커지고 있습니다. 국제적으로는 각국이 협력을 강화하며 무역과 문화 교류를 확대하고 있습니다. 국내에서는 공공 서비스 개선을 통해 국민 삶의 질 향상이 추진되고 있습니다. 주요 뉴스였습니다.".to_string(),
            voice_name: "Adam".to_string(),
        },
        DemoEntry {
            name: "🇪🇸 Noticiero".to_string(),
            role: "assets/audio/en_4.wav".to_string(),
            text: "Buenas noches, bienvenidos al noticiero. Hoy se han logrado avances importantes en innovación tecnológica, lo que impulsará el crecimiento económico. A nivel internacional, los países fortalecen la cooperación para ampliar el comercio y el intercambio cultural. En el ámbito nacional, continúan los esfuerzos para mejorar los servicios públicos y la calidad de vida. Estos fueron los titulares.".to_string(),
            voice_name: "Adam".to_string(),
        },
        DemoEntry {
            name: "🇫🇷 Journal".to_string(),
            role: "assets/audio/en_4.wav".to_string(),
            text: "Bonsoir et bienvenue au journal. Aujourd’hui, des progrès significatifs ont été réalisés dans le domaine de l’innovation technologique, favorisant la croissance économique. Sur le plan international, les pays renforcent leur coopération pour développer le commerce et les échanges culturels. Au niveau national, des efforts sont menés pour améliorer les services publics et la qualité de vie. Voilà pour les titres.".to_string(),
            voice_name: "Adam".to_string(),
        },
        DemoEntry {
            name: "🇩🇪 Nachrichten".to_string(),
            role: "assets/audio/en_4.wav".to_string(),
            text: "Guten Abend und willkommen zu den Nachrichten. Heute wurden wichtige Fortschritte im Bereich der technologischen Innovation erzielt, die das Wirtschaftswachstum fördern. Länder arbeiten weltweit zusammen, um Handel und kulturellen Austausch auszubauen. Im Inland verbessern sich die öffentlichen Dienstleistungen weiter. Das waren die wichtigsten Nachrichten.".to_string(),
            voice_name: "Adam".to_string(),
        },
        DemoEntry {
            name: "🇮🇹 Telegiornale".to_string(),
            role: "assets/audio/en_4.wav".to_string(),
            text: "Buonasera e benvenuti al telegiornale. Oggi sono stati compiuti importanti progressi nell’innovazione tecnologica, favorendo la crescita economica. A livello internazionale, i paesi rafforzano la cooperazione. A livello nazionale, i servizi pubblici continuano a migliorare. Queste erano le principali notizie.".to_string(),
            voice_name: "Adam".to_string(),
        },
        DemoEntry {
            name: "🇭🇺 Híradó".to_string(),
            role: "assets/audio/en_4.wav".to_string(),
            text: "Jó estét kívánunk, köszöntjük híradónkban. Ma jelentős előrelépések történtek a technológiai innováció terén. Nemzetközi szinten az országok erősítik az együttműködést. Belföldön javulnak a közszolgáltatások. Ezek voltak a fő hírek.".to_string(),
            voice_name: "Adam".to_string(),
        },
        DemoEntry {
            name: "🇷🇺 Новости".to_string(),
            role: "assets/audio/en_4.wav".to_string(),
            text: "Добрый вечер, вы смотрите новости. Сегодня достигнуты важные успехи в области технологических инноваций. На международной арене страны усиливают сотрудничество. Внутри страны продолжается улучшение государственных услуг. Это были главные новости.".to_string(),
            voice_name: "Adam".to_string(),
        },
        DemoEntry {
            name: "🇮🇷 اخبار".to_string(),
            role: "assets/audio/en_4.wav".to_string(),
            text: "شب بخیر، به بخش خبری خوش آمدید. امروز پیشرفت‌های مهمی در حوزه نوآوری فناوری حاصل شده است. در سطح بین‌المللی، کشورها همکاری‌های خود را تقویت می‌کنند. در داخل، خدمات عمومی در حال بهبود است. این‌ها مهم‌ترین اخبار بودند.".to_string(),
            voice_name: "Adam".to_string(),
        },
        DemoEntry {
            name: "🇸🇦 النشرة الإخبارية".to_string(),
            role: "assets/audio/en_4.wav".to_string(),
            text: "مساء الخير، أهلاً بكم في النشرة الإخبارية. اليوم تم تحقيق تقدم مهم في مجال الابتكار التكنولوجي. دولياً، تعزز الدول تعاونها. محلياً، تستمر الجهود لتحسين الخدمات العامة. كانت هذه أبرز الأخبار.".to_string(),
            voice_name: "Adam".to_string(),
        },
        DemoEntry {
            name: "🇵🇱 Wiadomości".to_string(),
            role: "assets/audio/en_4.wav".to_string(),
            text: "Dobry wieczór, witamy w wiadomościach. Dziś osiągnięto ważne postępy w dziedzinie innowacji technologicznych. Na arenie międzynarodowej kraje wzmacniają współpracę. W kraju poprawiają się usługi publiczne. To były najważniejsze wiadomości.".to_string(),
            voice_name: "Adam".to_string(),
        },
        DemoEntry {
            name: "🇵🇹 Noticiário".to_string(),
            role: "assets/audio/en_4.wav".to_string(),
            text: "Boa noite, bem-vindos ao noticiário. Hoje foram alcançados avanços importantes na inovação tecnológica. Internacionalmente, os países fortalecem a cooperação. A nível nacional, os serviços públicos continuam a melhorar. Estas foram as principais notícias.".to_string(),
            voice_name: "Adam".to_string(),
        },
        DemoEntry {
            name: "🇨🇿 Zprávy".to_string(),
            role: "assets/audio/en_4.wav".to_string(),
            text: "Dobrý večer, vítejte u zpráv. Dnes byly dosaženy významné pokroky v technologických inovacích. Na mezinárodní úrovni země posilují spolupráci. Doma se zlepšují veřejné služby. To byly hlavní zprávy.".to_string(),
            voice_name: "Adam".to_string(),
        },
        DemoEntry {
            name: "🇩🇰 Nyhederne".to_string(),
            role: "assets/audio/en_4.wav".to_string(),
            text: "God aften og velkommen til nyhederne. I dag er der opnået vigtige fremskridt inden for teknologisk innovation. Internationalt styrker landene samarbejdet. Nationalt forbedres de offentlige tjenester. Det var dagens vigtigste nyheder.".to_string(),
            voice_name: "Adam".to_string(),
        },
        DemoEntry {
            name: "🇸🇪 Nyheterna".to_string(),
            role: "assets/audio/en_4.wav".to_string(),
            text: "God kväll och välkommen till nyheterna. Idag har viktiga framsteg gjorts inom teknologisk innovation. Internationellt stärker länder samarbetet. Nationellt förbättras de offentliga tjänsterna. Det var dagens viktigaste nyheter.".to_string(),
            voice_name: "Adam".to_string(),
        },
        DemoEntry {
            name: "🇬🇷 Ειδήσεις".to_string(),
            role: "assets/audio/en_4.wav".to_string(),
            text: "Καλησπέρα σας, καλώς ήρθατε στις ειδήσεις. Σήμερα σημειώθηκαν σημαντικές πρόοδοι στην τεχνολογική καινοτομία. Σε διεθνές επίπεδο, οι χώρες ενισχύουν τη συνεργασία. Σε εθνικό επίπεδο, βελτιώνονται οι δημόσιες υπηρεσίες. Αυτές ήταν οι κύριες ειδήσεις.".to_string(),
            voice_name: "Adam".to_string(),
        },
        DemoEntry {
            name: "🇹🇷 Haber Bülteni".to_string(),
            role: "assets/audio/en_4.wav".to_string(),
            text: "İyi akşamlar, haber bültenine hoş geldiniz. Bugün teknolojik yenilik alanında önemli ilerlemeler kaydedildi. Uluslararası alanda ülkeler iş birliğini güçlendiriyor. Yurt içinde kamu hizmetleri iyileştiriliyor. Bunlar günün öne çıkan haberleriydi.".to_string(),
            voice_name: "Adam".to_string(),
        },
    ]
}

/// Cached demo list.
pub static ALL_DEMOS: std::sync::OnceLock<Vec<DemoEntry>> = std::sync::OnceLock::new();

/// Get a demo entry by its ID (format: `demo-0`, `demo-1`, etc.).
///
/// Mirrors the Python `_resolve_demo_entry()` from `app.py`.
pub fn get_demo_by_id(demo_id: &str) -> Option<DemoEntry> {
    let demos = all_demos();
    let index = demo_id.strip_prefix("demo-")?.parse::<usize>().ok()?;
    demos.get(index).cloned()
}

/// Resolve the full audio path for a demo entry.
///
/// The `role` field is a relative path like `assets/audio/zh_1.wav`.
/// This function resolves it relative to the TTS model voice directory.
/// If the file doesn't exist, returns the role as-is.
pub fn resolve_demo_audio_path(demo_id: &str) -> Option<std::path::PathBuf> {
    let demo = get_demo_by_id(demo_id)?;
    let voice_dir = crate::modules::tts::config::default_voice_dir();
    // The role is like "assets/audio/zh_1.wav" — extract just the filename
    let file_name = std::path::Path::new(&demo.role)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| demo.role.clone());
    let path = voice_dir.join(&file_name);
    if path.exists() {
        Some(path)
    } else {
        Some(std::path::PathBuf::from(file_name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_count_matches_jsonl_reference() {
        let demos = all_demos();
        assert_eq!(demos.len(), 29, "must match 29 demos from demo.jsonl");
    }

    #[test]
    fn first_demo_is_chinese_welcome() {
        let demos = all_demos();
        assert!(demos[0].name.contains("\u{6b22}\u{8fce}"));
        assert_eq!(demos[0].voice_name, "Junhao");
    }

    #[test]
    fn last_demo_is_turkish() {
        let demos = all_demos();
        assert!(demos[28].name.contains("Haber"));
        assert_eq!(demos[28].voice_name, "Adam");
    }

    #[test]
    fn all_demos_have_nonempty_text() {
        let demos = all_demos();
        for demo in &demos {
            assert!(!demo.text.is_empty(), "demo has empty text: {}", demo.name);
        }
    }

    #[test]
    fn all_demos_have_voice_mapping() {
        let demos = all_demos();
        let voices = super::super::presets::list_voice_names();
        for demo in &demos {
            assert!(
                voices.contains(&demo.voice_name.as_str()),
                "demo maps to unknown voice: {} -> {}",
                demo.name,
                demo.voice_name
            );
        }
    }
}
