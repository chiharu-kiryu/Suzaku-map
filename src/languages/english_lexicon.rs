// Project-authored offline vocabulary, in approximate common-use priority order.
// This is a curated baseline, not a downloaded corpus or a learned user history.
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

// Project-authored additions: everyday life, work, development and explicit
// inflections. Enumerate real forms; do not generate suffixes such as "runned".
// The dictionary appends this block after the established vocabulary ranks.
pub(super) const EXTENDED_WORDS: &str = "
breakfast groceries grocery restaurant receipt reservation subscription backpack
pharmacy kitchen luggage suitcase airport airline airplane flight boarding passport
visa departure departures arrivals gate station railway train subway underground tram
bus taxi tickets fare route journey trip tourism tourist hotel hostel accommodation
reception receptionist reservations booking bookings booked itinerary destinations
sightseeing museum gallery beach mountain river forest park garden lake village town
countryside neighborhood neighbourhood downtown nearby abroad overseas domestic
baggage aisle seat seating passenger passengers vehicle traffic bicycle bike cycling
motorcycle parking pedestrian crosswalk intersection crossing roundabout bridge tunnel
sidewalk pavement driving driver drivers rental renting rented rent apartment flat
bedroom bathroom living lounge hallway balcony ceiling floor furniture desk chair sofa
couch shelf shelves drawer cupboard wardrobe closet carpet curtain pillow blanket
towel mattress bed bedding laundry washing dryer detergent cleaning vacuum rubbish
garbage recycling trash bin dishes dishwasher sink fridge refrigerator freezer oven
stove microwave kettle toaster spoon fork knife knives bowl plate cup mug glass bottle
pot pan lid basket bag shopping shop supermarket market checkout cashier discount
coupon voucher refund purchase purchases purchased payment paid cash coin coins wallet
purse cashless invoice invoices bill bills billing budget expenses expensive
affordable bargain cheaper cheapest fee fees charges prices quantity stock sold
selling delivery delivered shipment shipping shipped parcel packaging courier tracking
postcode postal zip addresses recipient sender signature signed insurance warranty
guarantee replacement replaceable returnable refundable cancellation cancelled
canceled cancelling canceling subscriptions subscriber subscribers renewal renew
renewed membership members customers consumer consumers supplier vendor seller buyer
retailers retail wholesale retailer inventory meal meals snack snacks sandwich
sandwiches salad soup rice noodle noodles pasta pizza bread toast cereal oatmeal flour
butter cheese yogurt yoghurt milk cream egg eggs chicken beef pork fish seafood tofu
vegetable vegetables fruit fruits apple apples banana bananas oranges grape grapes
lemon lime peach peaches pear pears berry berries strawberry strawberries blueberry
blueberries potato potatoes tomato tomatoes onion onions carrot carrots garlic ginger
pepper peppers mushroom mushrooms lettuce cucumber broccoli spinach corn peas beans
lentils nuts almond almonds peanut peanuts walnut walnuts salt sugar honey oil vinegar
sauce spice spices sweet sour bitter spicy mild crispy crunchy frozen cooked cooking
cook recipe recipes ingredient ingredients bake baked baking boil boiled boiling fry
fried frying roast roasted roasting grill grilled grilling steam steamed steaming
vegetarian vegan allergy allergies allergic gluten dairy caffeine decaf tea juice soda
beer wine sparkling thirsty hungry delicious tasty taste flavor flavour flavors
flavours beverage beverages menus portion portions appetite healthy healthier
unhealthy nutrition protein vitamin vitamins exercising gym fitness workout workouts
yoga stretch stretching jogging hiking swimming swim walking athlete athletic sport
sports football soccer basketball tennis badminton volleyball baseball golf
competition competitive trainer coach coaching teammates matches tournament score
scores scored winning winner winners lose losing appointments clinic hospital doctor
nurse dentist dental medical medicine medication prescription healthcare headache
stomach stomachache fever cold cough coughing sore throat pain painful hurt injury
injured recovery recovered recovering rest resting tired exhausted fatigue stress
stressed stressful relax relaxed relaxing calm comfortable uncomfortable emergency
urgent urgency ambulance symptoms symptom treatment treated treating therapy therapist
patient patients sick illness disease wellness physical mental emotional emotion
emotions feeling feelings worried anxious anxiety upset disappointed disappointment
frustrated frustration grateful gratitude pleased pleasure pleasant excitement curious
curiosity surprised surprise surprising confident confidence nervous patience
impatient proud pride lonely delighted delight smile smiling laughed laughing laughter
cried crying tears tear relationship relationships friendship friendly colleague
colleagues neighbor neighbors neighbour neighbours partner partners spouse husband
wife parents mother father mom dad grandmother grandfather grandparents brother sister
siblings sibling son daughter children baby babies uncle aunt cousin cousins nephew
niece relative relatives married marriage wedding anniversary celebration celebrate
celebrated celebrating invitation invitations invited inviting guest guests visitor
visitors greeting greetings congratulations congrats farewell goodbye bye cheers
weekdays weekday midnight noon lunchtime bedtime sunrise sunset daylight season
seasons spring autumn fall winter weather sunny cloudy clouds rain rainy raining snow
snowy snowing wind windy storm stormy thunder lightning fog foggy warm warmer cool
cooler colder heat heating freezing forecast humidity climate umbrella raincoat coat
jacket sweater shirt blouse dress skirt trousers pants jeans shorts socks shoes boots
sandals sneakers trainers glove gloves hat cap scarf belt pocket pockets buttons
zipper sleeve sleeves uniform suit casual formal fashion fashionable fabric cotton
wool leather material materials colors colour colours purple pink brown grey gray
silver gold golden washed cleaner dirty dirt dust dusty wet dry drying repair repairs
repaired repairing broken damage damaged electricity electric electrical gas plumbing
plumber electrician broadband router modem cable charger charging charged batteries
adapter adaptor plug unplug plugged connected disconnected connections signals
bluetooth wifi ethernet cellular roaming smartphone tablet laptop notebook monitors
headphone headphones earphone earphones earbud earbuds speaker speakers microphone
webcam photos photograph photographs photography videos recording recordings
screenshot screenshots screens displays resolution brightness contrast mute muted
unmute silent silence vibration alarm reminder reminders notifications ringtone
calendars contacts messages conversations chats chatting calls calling called answered
answering voicemail inbox outbox emails attachments attaching forwarded forwarding
replied replying responses responsive responding reading reader readers readable
unread unreadable writes writer writers wrote rewrite rewriting rewritten proofread
proofreading grammar grammatical punctuation sentences paragraph paragraphs articles
chapter chapters books stories novel novels fiction nonfiction poetry poem poems
literature literary authors authoring journal journals journalism newspaper newspapers
magazine magazines headline headlines publication publications published publishing
publisher editor editors editing edited editorial revisions revise revised revising
drafts outline outlines summaries summarize summarized summarizing summarise
summarised summarising translations translated translating translator translators
interpretation interpreter interpret interpreted vocabulary dictionary dictionaries
lexicon phrase phrases idiom idioms syllable syllables pronunciation pronounce
pronounced accent accents fluent fluency bilingual multilingual foreign beginner
beginners intermediate lesson lessons courses class classes classroom lecture lectures
lecturer tutor tutoring teacher teachers professor universities college colleges
campus schools students graduate graduates graduated graduating graduation
undergraduate postgraduate degrees diploma certificate certificates qualification
qualifications assignments homework exam exams examination quiz quizzes tests tested
practices practicing practise practising practiced practised studies studied studying
learned learnt learner learners teach teaches taught teaching understanding understood
misunderstand misunderstood explained explaining explanation explanations examples
demonstrate demonstrated demonstrating demonstration instructions instruct instructed
instructing tutorials manuals guides guided guiding references referenced referencing
researcher researchers researched researching experiments experimental evidence
hypothesis hypotheses theory theories theoretical practical practically analyses
analyst analytical analyzed analyzing analyse analysed analysing evaluation evaluated
evaluating observation observations observed observing measurement measurements
measured measuring results outcome outcomes finding findings conclusion conclusions
conclude concluded concluding recommendation recommendations recommend recommended
recommending reports reported reporting presentation presentations presented
presenting presenter presenters audience participants participant participate
participated participating participation discussions discussed discussing debate
debates debated debating agreement agreements agreed agreeing disagree disagreed
disagreeing disagreement compromise compromises negotiate negotiated negotiating
negotiation negotiations decisions decided deciding choices chooses choosing chose
chosen options optional alternatives priorities prioritize prioritized prioritizing
prioritise prioritised prioritising importance essential necessity requirements
required requiring requests requested requesting proposal proposals propose proposed
proposing plans planned planning strategy strategies strategic roadmap milestone
milestones deadline deadlines timeline timelines schedules scheduled scheduling
reschedule rescheduled rescheduling availability unavailable meetings conference
conferences agenda agendas minutes attendee attendees attended attending attendance
organized organizing organiser organizer organisation organisations organizations
management managing managed managers leader leaders leadership leading led supervise
supervisor supervision teamwork collaboration collaborate collaborated collaborating
collaborative coordinate coordinated coordinating coordination coordinator
coordinators workflow workflows tasks workload workloads responsibility
responsibilities responsible ownership owners stakeholder stakeholders department
departments division divisions units roles employee employees employer employers
employment jobs career careers occupation occupations professionals profession
industry industries industrial businesses enterprise enterprises entrepreneur startup
startups companies corporation corporate offices remotely hybrid flexible flexibility
overtime vacation holidays leaves salary salaries wage wages payroll bonus bonuses
benefits contract contracts contractor contractors freelance freelancer freelancers
interview interviews interviewing interviewed applicant applicants applications
recruitment recruit recruited recruiting resume portfolio portfolios experiences
experienced inexperienced skill skills skilled ability abilities capable capability
capabilities competency reviews reviewed reviewing reviewer reviewers approvals
approved approving authorization authorisation authorized authorised permissions
permitted permit permits restriction restrictions restricted accessible accessibility
inaccessible secured securing confidential confidentiality sensitive sensitivity
protection protected protecting prevention prevent prevented preventing precaution
precautions safety unsafe risks risky assessment assess assessed assessing audit
audits audited auditing compliance compliant policies procedures procedural regulation
regulations guideline guidelines standards standardized consistency consistently
inconsistent reliability reliably unreliable stability unstable qualitative
quantitative accuracy accurate accurately inaccurate precision precise precisely
approximate approximately approximation estimates estimated estimating estimation
forecasts forecasting expectation expectations expected expecting behavior behaviour
behavioral behavioural performant efficiency efficiently inefficient optimization
optimize optimized optimizing optimise optimised optimising improvements improved
improving progressed progressing productive productivity products producer produce
produced producing releases released releasing launched launching versions versioning
updates updated updating upgrade upgrades upgraded upgrading downgrade migration
migrate migrated migrating incompatible supported unsupported supports supporting
supporter supportable uptime downtime outage outages incident incidents investigate
investigated investigating diagnosis diagnose diagnosed diagnosing diagnostic
diagnostics troubleshoot troubleshooting workaround workarounds solutions resolved
resolving resolutions fixes fixed fixing bugs buggy defect defects failures failing
failed succeed succeeded succeeding successfully unsuccessful acceptance accepts
rejected reject rejecting rejection validation validated validating verification
verify verified verifying confirmed confirming revert reverted reverting rollback
rollbacks restored restoring restoration recoverable backup backups backed backing
snapshot snapshots replica replication replicate replicated replicating redundancy
redundant archive archives archived archiving storage storing stored stores disk disks
drives filesystem directories folders files filename filenames extensions paths
absolute symlink symbolic links linked linking unlink unlinked unlinking mount mounted
mounting unmount partition partitions executable execute executed executing execution
readonly writable readability writability binary binaries byte bytes bit bits buffers
buffered buffering streams streaming streamed channel channels pipe pipes pipeline
pipelines inputs outputs parameters arguments flags environments variables constants
configurations configurable configured configuring defaults mandatory enabled enabling
disabled disabling toggled toggle toggles toggling switches switching keyboards keys
keypress keystroke keystrokes keybinding keybindings shortcuts hotkey hotkeys keymap
layouts mapping mappings mapped capslock numlock modifier modifiers alt compose
composition composing preedit candidates suggestions prediction predictions predictive
predict predicted predicting completions autocomplete autocompletion autocorrect
correction corrections corrected correcting typo typos typing typed types datatype
datatypes schema schemas records fields values strings substring substrings prefix
prefixes suffix suffixes characters codepoint codepoints unicode utf encoding
encodings encoded encode encodes decode decodes decoded decoding normalize normalized
normalization normalisation denormalized escaped escaping delimiter delimiters
separator separators whitespace indentation indent indented newline newlines tab tabs
spaces trailing trim trimmed trimming truncate truncated truncating truncation lengths
widths heights rectangle rectangles bounds boundary boundaries bounding coordinates
cursor cursors caret carets selections selected selecting highlighted highlighting
focused focusing unfocused blur blurred draggable drag dragged dragging dropped
dropping resized resizing scaling scale scaled zooming zoomed scrolled scrolling
viewport viewports popup popups dialog dialogs modal modals tooltips toolbars sidebar
sidebars submenu submenus checkbox checkboxes radio slider sliders dropdown
placeholder placeholders labels icons glyph glyphs fonts typeface typefaces typography
typographic antialiasing raster rasterization rendering renderer rendered renderers
shader shaders shading texture textures atlas atlases vertex vertices triangle
triangles geometry geometric graphics graphical animation animations animate animated
animating transition transitions frames framerate refreshed refreshing repaint
repainting redraw redrawing surfaces compositing compositor transparency transparent
opaque opacity contrasting themes theming palette palettes gradient gradients shadow
shadows borders margin margins padding spacing alignment align aligned aligning
overlap overlapping overflow overflowing clipped clip clipping clipboard frontend
backend frameworks libraries module modules components architecture architectural
architect abstraction abstractions abstract concrete implementations implemented
implementing interfaces trait traits inheritance encapsulation polymorphism generic
generics monomorphization compiler compilers compiled compiling compilation linker
assembler assembly runtime runtimes executables debugger debuggers debugging debugged
logging logger logs logged trace traces tracing traced telemetry metrics metric
monitoring monitored profiling profiler profiles benchmark benchmarks benchmarking
benchmarked throughput latency bottleneck bottlenecks contention caches cached caching
uncached invalidation invalidate invalidated invalidating memoization incremental
increments increment decrement decrements counter counters algorithm algorithms
algorithmic complexity complicated complex recursion recursive iterate iterated
iterating iteration iterations iterator iterators iterable collections vector vectors
array arrays lists queue queues enqueue dequeue deque stack stacks heap heaps maps
hashmap hashmaps hash hashing hashed hashset sets sorted sorting searches searching
lookup lookups indexes indices indexed indexing trie tries tree trees graph graphs
node nodes edges depth breadth traversal traverse traversing visited visiting cycles
cyclic acyclic directed undirected linear logarithmic quadratic exponential polynomial
bounded unbounded finite infinite max min average median percentile percentiles
distributions probability probabilities probabilistic randomness seed seeds seeded
deterministic nondeterministic entropy threads threaded threading multithreading
concurrency concurrent concurrently parallelism parallelize parallelized parallelizing
synchronization synchronized synchronizing synchronous asynchronous async await
awaiting blocking nonblocking blocked unblock unblocked deadlock deadlocks race races
racing atomic atomics atomicity mutex mutexes semaphore semaphores locks locked
locking unlocked unlocking latch latches barrier barriers worker workers stealing
scheduler threadpool pool pools futures promise promises callback callbacks closure
closures functions functional pure impurity immutable mutable mutability immutability
borrow borrowed borrowing lifetime lifetimes scopes scoped resources allocation
allocate allocated allocating allocator deallocate deallocated deallocation collector
leak leaks leaked leaking pointer pointers offset offsets unaligned memories underflow
segmentation fault faults panic panics panicked panicking unwind unwinding abort
aborted aborting exception exceptions exceptional caught catching throw thrown throws
throwing errors erroneous handling handles handled handler handlers unrecoverable
fatal networks networking packet packets protocols sockets connecting disconnect
disconnecting reconnect reconnected reconnecting endpoint endpoints header headers
bodies payload payloads queries querying queried parser parsers parsed parsing
syntactic semantic semantics serialize serialized serializing serialization
deserialize deserialized deserialization marshal marshaling unmarshal unmarshalling
yaml xml toml validator validators sanitizer sanitization sanitize sanitized
encryption encrypt encrypted encrypting decrypt decrypted decrypting decryption
authentication authenticate authenticated authenticating credentials credential secret
secrets tokens cryptography cryptographic signatures signing unsigned checksum
checksums digest digests fingerprint fingerprints salted hashes databases
transactional transaction transactions commits committed committing rolled isolation
isolated isolate isolating durability durable persistence persistent persisted
persisting relational relation relations tables column columns row rows constraint
constraints constrained uniqueness joins joining joined union unions intersect
aggregate aggregation grouped grouping counts counted counting sum sums summing summed
ordering ordered pagination paginated paginate filtering filters filtered ascending
descending null nullable nullability empty emptiness deployment deployed deploying
deployments orchestration orchestrator containers containerized virtualization
virtualized machines images builds built building artifact artifacts packages packaged
distribute distributed distributing bundle bundles bundled bundling dependencies
dependent depend depending repositories branches branched branching merges merged
merging rebase rebased rebasing cherry patches patched patching diff differences
conflict conflicts conflicting conflicted remotes upstream downstream origin origins
clone clones cloned cloning fetch fetched fetching pushes pushed pushing pulls pulled
pulling regression regressions regress regressed regressing integrated integrating
testable mock mocks mocked mocking fixture fixtures assertion assertions assert
asserted asserting coverage covered covering uncovered reproducible reproduce
reproduced reproducing reproduction reproducer intermittent intermittently flaky
flakiness stabilize stabilized stabilizing stabilization bootstrap bootstrapping
initialized initializing initialization initialise initialised initialising
initialisation teardown cleanup cleanups cleaned disposal dispose disposed disposing
shutdown starting started stopped stopping restarted restarting retried retrying
retries timeouts timed cancelable cancellable interrupted interrupt interrupting
interruption delayed delaying delays debounced debounce debouncing throttle throttled
throttling coalesce coalesced coalescing batch batches batched batching pending
completed incomplete finished finishing ongoing arrives arrived arriving brings
bringing brought buys buying bought goes going went gone comes coming came gets
getting got gotten gives giving gave given takes taking took taken makes making made
sees seeing saw seen knows knowing knew known thinks thinking says saying said tells
telling told speaks speaking spoke spoken eat eats eating ate eaten drink drinks
drinking drank drunk sleeps sleeping slept keeps keeping kept leaving meet meets met
feels felt hears hearing heard finds loses runs ran sit sits sitting sat stands
standing stood falls falling fell fallen holds holding held pays paying spends
spending spent sells sends breaks breaking broke begins began begun becomes becoming
became wear wears wearing wore worn grows growing grew grown draw draws drawing drew
drawn drove driven forget forgets forgetting forgotten remembers remembering
remembered catches understands reads travels traveled travelled traveling travelling
decides hopes hoped hoping moves moved moving loves loved loving uses used using tried
trying worries worrying carry carries carried carrying copies copied copying replies
stops prefers preferred preferring occur occurs occurred occurring admit admits
admitted admitting permitting smaller smallest big bigger biggest largest easier
easiest harder hardest faster fastest slower slowest stronger strongest weak weaker
weakest higher highest lowest earlier earliest shorter shortest rich richer richest
poor poorer poorest happier happiest busy busier busiest bad worse worst farther
farthest furthest
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
    ("could we", &["reschedule", "meet", "discuss"]),
    ("please attach", &["the", "your"]),
    ("please restart", &["the"]),
    ("please run", &["the"]),
    ("i'll bring", &["the", "some"]),
    ("let's grab", &["some", "a"]),
    ("make a", &["reservation", "booking", "payment"]),
    ("catch the", &["train", "bus", "flight"]),
    ("book a", &["table", "room", "flight"]),
    ("the deployment", &["is", "was"]),
    ("the database", &["is", "needs"]),
    ("the server", &["is", "was"]),
    ("regression", &["tests", "testing"]),
    ("keyboard", &["shortcuts", "layout", "input"]),
    ("transaction", &["isolation", "log"]),
    ("authentication", &["failed", "required"]),
];

pub(super) const PHRASE_ENDINGS: &[(&str, &[&str])] = &[
    ("thank you", &["for your help", "very much"]),
    ("how", &["are you", "can I help"]),
    ("see", &["you later"]),
    ("i would like", &["to know", "to ask"]),
];

// Complete, project-authored collocations. Word and sentence suggestions share
// these continuations so typing a separator or the next prefix does not lose them.
pub(super) const SENTENCES: &[&str] = &[
    "hello, how are you?",
    "hello, nice to meet you.",
    "help me with this, please.",
    "good morning, how are you?",
    "good morning, have a nice day.",
    "good evening, nice to see you.",
    "thank you for your help.",
    "thank you very much.",
    "thank you for the update.",
    "hello world, nice to meet you.",
    "thanks for your help.",
    "thanks for the update.",
    "please send me the details.",
    "please send me a message.",
    "please check the latest version.",
    "please check the details.",
    "please let me know.",
    "how are you?",
    "how can I help you?",
    "how are things going?",
    "let me know what you think.",
    "let me check the details.",
    "let me know if you need help.",
    "see you tomorrow.",
    "see you later.",
    "i would like to know more.",
    "i would like to ask a question.",
    "i am happy to help.",
    "i am working on it.",
    "we can discuss it later.",
    "we can try again.",
    "sorry for the delay.",
    "sorry about that.",
    "welcome to the team.",
    "looking forward to hearing from you.",
    "thank you for your feedback.",
    "thank you so much.",
    "thanks for letting me know.",
    "i'm happy to help.",
    "i'm working on it.",
    "i'm not sure yet.",
    "i'll send you the details.",
    "i'll check the latest version.",
    "i've been working on this.",
    "i've attached the file.",
    "we're working on it.",
    "we're ready to start.",
    "you're welcome.",
    "you're right about that.",
    "that's a good idea.",
    "let's try it again.",
    "let's take a look.",
    "could you please check this?",
    "could you send me the details?",
    "could you share the link?",
    "can you help me with this?",
    "can you share the file?",
    "would you like to try again?",
    "what do you think?",
    "what do you need?",
    "do you have time?",
    "do you want to try again?",
    "i would like to schedule a meeting.",
    "i want to know more.",
    "i want to try again.",
    "i need to check the details.",
    "we need to test this.",
    "we need to discuss the details.",
    "please review the changes.",
    "please update the documentation.",
    "please share the link.",
    "please try again.",
    "please let me know if you need help.",
    "open the file.",
    "open the settings panel.",
    "save the file.",
    "run the tests.",
    "fix the bug.",
    "check the error message.",
    "the latest version is available.",
    "could we reschedule the meeting?",
    "could we meet tomorrow?",
    "please attach the invoice.",
    "please attach the receipt.",
    "i'll bring the groceries home.",
    "i'll bring some snacks.",
    "let's grab some lunch.",
    "let's grab a coffee.",
    "i need a receipt, please.",
    "i need to make a reservation.",
    "i'd like to book a table.",
    "i'd like to change my reservation.",
    "do you have any vegetarian options?",
    "do you have a table for two?",
    "where is the nearest pharmacy?",
    "where is the train station?",
    "what time does the train leave?",
    "what time is breakfast?",
    "could you confirm the address?",
    "could you send me the receipt?",
    "i'm on my way.",
    "i'm running a little late.",
    "i'll be there in a few minutes.",
    "i'll call you when I arrive.",
    "have a great weekend.",
    "see you at the station.",
    "the package has arrived.",
    "the appointment is confirmed.",
    "please confirm your availability.",
    "please check the attachment.",
    "please find the attached report.",
    "could you clarify the requirements?",
    "let's discuss the schedule.",
    "let's review the proposal.",
    "i'll update the documentation.",
    "i'll follow up tomorrow.",
    "please restart the server.",
    "please run the regression tests.",
    "please check the configuration.",
    "please back up the database.",
    "the deployment is ready.",
    "the deployment is complete.",
    "the database connection is working.",
    "the server is running.",
    "the issue is reproducible.",
    "the regression tests passed.",
    "i can reproduce the issue.",
    "let's investigate the error.",
];
