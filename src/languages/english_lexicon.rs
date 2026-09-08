// Project-authored offline vocabulary, in approximate common-use priority order.
// This is a compact baseline, not a downloaded corpus or a learned user history.
pub(super) const WORDS: &str = "
the be to of and a in that have it for not on with he as you do at this
but his by from they we say her she or an will my one all would there their
what so up out if about who get which go me when make can like time no just
him know take people into year your good some could them see other than then
now look only come its over think also back after use two how our work first
well way even new want because any these give day most us is are was were
been being has had did does done may might must should need very much more
hello help here please thank thanks welcome world hope happy home today
tomorrow yesterday morning afternoon evening tonight soon later again next
yes okay sure sorry great nice fine best better really right together
send sent sending message meeting email call reply answer ask tell let
check try find read write learn start stop continue finish understand
wanting waiting wait ready available possible probably prefer support
project program progress problem process provide product production
complete completion candidate input method keyboard language model local
information important international computer change build test testing
something someone anything anyone everything everyone nothing nobody
before during between through under above around against without within
while still both each every either neither such another same own too
many few less least enough almost always often sometimes never once twice
where why whose whom whether although however since until unless though
am isn't aren't wasn't weren't don't doesn't didn't can't couldn't
won't wouldn't shouldn't haven't hasn't hadn't it's that's there's
here's what's who's let's i'm i'll i've i'd you're you'll you've you'd
we're we'll we've we'd they're they'll they've they'd he's she's
you’re you’ll you’ve don’t doesn’t didn’t can’t won’t it’s let’s i’m i’ll
able access account action activity actually add address agree ahead
allow already along alone amount another application apply appointment
area arrive article attention audio away basic beautiful become begin
believe benefit book break bring business buy calendar camera care case
catch cause certain chat choose city clear client close code coffee color
command comment common company compare confirm connect connection contact
content context control conversation correct cost country course create
current customer data date deal decide decision define delay delete deliver
design detail develop development device different difficult dinner direct
direction discuss discussion document download drive early easy edit effect
effort else end enjoy enter environment error especially event example
excellent expect experience explain extra face fact family far fast favorite
feature feedback feel file final finally fix follow following food form forward
free friend full fun function future general glad goal group grow guess guide
happen hard hear heart height high history hold hour house idea image include
increase indeed instead interest issue job join keep key kind last late lead
leave left letter life light line link list listen little live log long longer
longest lot love low lunch machine main major manage manager map mark matter
maybe mean member mention minute miss moment money month move music name near
necessary network news night note notice number offer office old online open
operation opportunity option order organize original outside output page pair
part particular party pass past pause pay person personal phone photo pick
picture place plan play point policy position power practice prepare present
press pretty previous price private professional profile public publish pull
purpose push put quality query question quick quickly quite reach real reason
receive recent record red reduce reference remember remove repeat replace
report request require research reset resolve response restart result return
review room round rule run safe save schedule school screen search second
section select selection self sell sense separate serious server service set
setting settings several share short show side sign simple single site size
sleep small social solution solve sort sound source space speak special speed
spend staff stage stand standard state stay step story strong student study
subject success suggest suggestion summary summer switch system table talk
task team technology term text thing thought thousand three ticket title
tomorrow tool top topic total touch toward track training travel treat true
turn type unit university update upload useful user usual value version video
view visit voice walk watch water week weekend weight whatever white whole
wide win wish word working works working world worry worth wrong young
accept accepted accepting active actual additional adjust admin advanced
advice age alternative analyze analysis announce anything appear appreciate
approach appropriate approval approve argument arrange arrival assignment
assistance assistant assume attach attached attachment attempt attend author
automatic automatically background balance bank base battery beginning behind
below birthday black block blue board body border bottom bound box branch
browser buffer bug button cache cancel capacity capture card careful center
central challenge character charge cheap child choice clean click clock cloud
collect collection comfort commit communicate communication community
compatible compatibility compile component concern condition configuration
configure confirm confirmation consider consistent constant consume container
content context contribute copy count couple cover crash credit critical
cross daily dark database debug default degree dependency deploy describe
description desktop destination detect difference digital directory disable
display distance distribution divide double draft drop due duplicate dynamic
edge education efficient element enable encourage energy engine enjoy entire
entry equal escape estimate evaluate exactly except exchange excited exercise
exist existing expand expense experiment export expression extend extension
external fail failure familiar feedback field filter financial flag focus
folder font force forgot format found frame framework fresh front fully
further generate generation global green ground handle hardware health helpful
hidden highlight holiday horizontal host hot huge human icon identity ignore
imagine immediately implement implementation improve improvement index
indicate individual initial initialize insert inside install installation
instance instruction integer integrate integration interface internal
internet introduce introduction invalid investigation invite item javascript
json jump knowledge label large larger latest launch layer layout learning
length level library license likely limit linux load loading location lock
login loop lost lower maintain maintenance manual match maximum measure medium
memory menu merge minimum missing mobile mode modify monitor mouse multiple
native natural nearly negative normal notification notify object observe
obtain obvious offline operate opposite orange organization overview owner
package panel parallel parameter parent parse password patch path pattern
perform performance perhaps permission persist platform plugin positive
potential prefer preference preview primary print priority privacy probably
procedure process processing project prompt property protect protocol provider
pull python random range rate rather raw react reality recognize recover
reduce refresh register regular related release reliable remain remote rename
render repeat repository represent require requirement resize resource respect
respond restore retry reverse revision risk role root rotate running rust
sample scan scenario scope script scroll secure security seem send sentence
session shape shift shortcut signal similar simplify situation slow smooth
socket software solid sorry sort specific speech spell spelling stable status
store stream string structure style submit successful sufficient surface
symbol synchronize syntax target technical temperature temporary terminal
thank theme thread timeout tiny token toolbar tooltip topic traditional
transfer transform translate translation transport trouble trust tutorial
typical unable undo unexpected unique unknown unlock unusual upper usage
valid validate variable various vertical virtual visible visual volume warning
website welcome wheel width window windows wireless wonderful workspace
write writing written yellow zero zoom English Japanese Chinese Monday Tuesday
Wednesday Thursday Friday Saturday Sunday January February March April June
July August September October November December America Europe Asia London
Tokyo Beijing OpenAI Llama Suzaku API JSON GPU CPU USB HTTP HTML CSS SQL
";

// Longest matching suffix wins. Only these explicit collocations produce offline
// next-word suggestions; unknown contexts have no made-up fallback phrases.
pub(super) const NEXT_WORDS: &[(&str, &[&str])] = &[
    ("thank you", &["for", "very", "so"]),
    ("thanks for", &["your", "the", "helping"]),
    ("thank you for", &["your", "the", "helping"]),
    ("would like", &["to", "a", "some"]),
    ("looking forward", &["to"]),
    ("look forward", &["to"]),
    ("let me", &["know", "check", "try", "see"]),
    ("let us", &["know", "start", "discuss"]),
    ("how are", &["you", "things"]),
    ("how can", &["I", "we", "you"]),
    ("could you", &["please", "send", "help", "check"]),
    ("can you", &["please", "help", "send", "check"]),
    ("as soon as", &["possible"]),
    ("see you", &["later", "soon", "tomorrow"]),
    ("nice to", &["meet", "see"]),
    ("i am", &["going", "happy", "sorry", "working"]),
    ("i have", &["a", "been", "the", "some"]),
    ("i will", &["send", "check", "try", "be"]),
    ("i think", &["that", "we", "it", "you"]),
    ("we can", &["use", "try", "start", "discuss"]),
    ("please send", &["me", "the", "your"]),
    ("send me", &["the", "a", "your"]),
    (
        "good",
        &[
            "morning",
            "afternoon",
            "evening",
            "night",
            "luck",
            "mood",
            "move",
        ],
    ),
    ("hello", &["world", "there", "everyone"]),
    ("thank", &["you"]),
    ("thanks", &["for", "again"]),
    ("please", &["send", "check", "let", "try", "help", "review"]),
    ("how", &["are", "can", "do", "to"]),
    ("what", &["is", "are", "do", "about"]),
    ("where", &["is", "are", "can", "do"]),
    ("when", &["will", "can", "is", "do"]),
    ("why", &["is", "not", "do"]),
    ("i", &["am", "would", "think", "have", "will"]),
    ("we", &["can", "are", "will", "have", "need"]),
    ("you", &["can", "are", "have", "will", "need"]),
    ("they", &["are", "have", "will", "can"]),
    ("it", &["is", "was", "will", "can"]),
    ("this", &["is", "will", "can", "project"]),
    ("that", &["is", "was", "would", "sounds"]),
    ("see", &["you", "the", "if"]),
    ("need", &["to", "a", "the", "help"]),
    ("want", &["to", "a", "the"]),
    ("going", &["to", "well"]),
    ("happy", &["to", "birthday", "with"]),
    ("sorry", &["for", "about", "to"]),
    ("looking", &["forward", "for", "at"]),
    ("look", &["forward", "at", "for"]),
    ("let", &["me", "us"]),
    ("let's", &["start", "try", "go", "discuss"]),
    ("best", &["regards", "wishes", "way"]),
    ("have", &["a", "the", "been", "some"]),
    ("there", &["is", "are", "was"]),
    ("your", &["help", "time", "feedback", "message"]),
    ("input", &["method", "mode", "text"]),
    ("language", &["model", "support", "settings"]),
    ("project", &["is", "needs", "will"]),
    ("meeting", &["at", "on", "with"]),
    ("pull", &["request"]),
    ("source", &["code", "file"]),
    ("user", &["interface", "experience", "input"]),
    ("open", &["the", "source", "a", "file"]),
    ("on", &["the", "my", "your", "Monday"]),
    ("in", &["the", "a", "this", "my"]),
    ("for", &["the", "your", "a", "me"]),
    ("to", &["the", "be", "do", "make", "see"]),
];

pub(super) const PHRASE_ENDINGS: &[(&str, &[&str])] = &[
    ("thank you", &["for your help", "very much"]),
    ("how", &["are you", "can I help"]),
    ("see", &["you later"]),
    ("i would like", &["to know", "to ask"]),
];
