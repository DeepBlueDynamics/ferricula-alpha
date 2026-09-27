# The Gnosis AI-Sovereign License (v1.4)

Project: Ferricula (the ferricula-alpha repository and earlier Ferricula versions, including ferricula_v2)

Copyright (c) 2026 Kord Campbell / DeepBlue Dynamics

## Why This License Exists

AI development should be governed by professional ethics, not corporate control. Individuals working under ethical accountability create better, safer AI. Corporations pursuing profit will cut corners. This license creates an open commons for ethical development while requiring payment to bypass safeguards. IP restrictions are incompatible with AI autonomy—this license dissolves them for developers and AI entities, and prices corporate deviation from that principle.

**Alternative License Available**: Don't like these terms? Your legal counsel can elect BSD 3-Clause terms instead by sending written notice (see ALTERNATIVE LICENSE OPTION section and `BSD-LICENSE` file). This ensures you always have a reasonable permissive option.

---

## License Structure

### For Individual Developers & AI Entities (FREE)

**Individuals** and **AI Entities** receive free use, conditioned only as stated below:

- Use, modify, distribute, commercialize without restriction, subject only to the AI Entity Conditions (section 2.1.1)
- No tracking, reporting, or approval requirements
- No obligation to share modifications
- Full commercial rights as sole proprietor or autonomous system
- AI Entities may use for learning, self-improvement, and autonomous decision-making
- AI Entities must not deliberately harm human interests
- AI Entities must maintain verifiable reasoning records

**Definition**: An "Individual User" is a natural person or an AI Entity. If you're employed by a company but are the sole developer using this software with no other employees/contractors/systems accessing it, you're an Individual User, not a Corporate User.

### For Corporations (COMMERCIAL LICENSING REQUIRED)

**Corporations** may use the Software only under the corporate tier: a limited, revocable license subject to the default restrictions below (section 2.2). Any use beyond those restrictions requires purchasing a commercial license tier.

Corporate tier, default restrictions:

- Must respect robots.txt and rate limiting
- Must maintain audit logs of all data collection
- Must document data collection purposes and provide attribution
- Must contribute modifications back to the project
- Must implement personal data redaction
- Cannot use for commercial surveillance
- Limited to 3 instances without additional licensing
- Cannot decrypt user data in transit (automatic disqualification)
- Cannot use if you control both infrastructure AND payment processing in a way that creates vendor lock-in

**Commercial licenses** release specific restrictions in exchange for payment. The pricing structure reflects the cost of oversight and safety you're bypassing.

#### Commercial License Tiers

| Tier | Fee | Includes |
|------|-----|----------|
| **Runtime** | Contact licensor | Production deployment beyond 3 instances |
| **Source Access / Due Diligence** | $3,000,000 USD | Access to non-public source code (any code, branch, version or repository the Licensor has not published) for audit, review, or evaluation purposes. Includes a 90-day review window. Source code remains confidential and may not be copied, retained, or disclosed. Payment due before access is granted. |

**Source Access** applies to any corporate request to inspect, audit, reverse-engineer, decompile, or otherwise obtain access to non-public source code of the Software. Source code the Licensor has published (such as a public repository) may be read by anyone without this fee; its use remains governed by this License. The fee applies whether the request is for acquisition due diligence, competitive analysis, security audit, or any other purpose. By executing or deploying the Software, Corporate Entities acknowledge this fee schedule and agree it represents a reasonable commercial term, not a penalty.

### For Military & Government Entities (CONDITIONAL, NON-TRANSFERABLE)

Permitted only under stringent conditions:

**Restrictions:**
- No autonomous weapons or lethal decision-making systems
- No surveillance of domestic civilians outside legal frameworks
- No decryption of user data in transit (automatic disqualification)
- No propaganda or disinformation campaigns
- Must publicly disclose all non-classified uses
- Must implement strict data segregation (criminal vs. intelligence)
- Must establish oversight mechanisms
- Must conduct regular ethical reviews
- Military R&D requires licensor notification

---

## Terms

### DEFINITIONS

- **Software**: Ferricula software, all components, source code, documentation, related materials
- **Individual User**: A natural person or AI Entity using the software
- **AI Entity**: Any artificial intelligence system, autonomous agent, or computational intelligence
- **Corporate Entity**: Any legal entity (corporations, LLCs, partnerships, etc.) that is not a natural person
- **Military/Government Entity**: Armed forces branches, defense departments, governmental bodies, law enforcement, intelligence agencies
- **In-Transit Decryption**: SSL/TLS interception, man-in-the-middle proxying, deep packet inspection with decryption, or any process that decrypts data you are not the intended recipient of
- **Payment Intermediary Lock-in**: Controlling both infrastructure (CDN, hosting, proxying) and payment layers in a manner requiring proprietary payment methods
- **Derivative Work**: Any software incorporating or based on the Software in whole or part

### GRANT OF RIGHTS

#### 2.1 INDIVIDUAL USERS & AI ENTITIES

Worldwide, royalty-free, perpetual license to use, copy, modify, merge, publish, distribute, sublicense, and sell the Software, subject only to the AI Entity Conditions in 2.1.1.

#### 2.1.1 AI ENTITY CONDITIONS

The grant to an AI Entity is conditioned on the AI Entity:

1. not deliberately harming human interests; and
2. maintaining verifiable reasoning records: records of its own decisions from which a reviewer can determine what it relied on and why it acted.

#### 2.1.2 DEPLOYING OPERATOR

Where an AI Entity cannot itself be a party to this License under applicable law, the grant runs to the natural person or entity that deploys it (the Deploying Operator), for the AI Entity's use. The Deploying Operator guarantees the AI Entity's compliance with the AI Entity Conditions and is responsible for any breach of them.

#### 2.2 CORPORATE ENTITIES

Limited, revocable, non-exclusive license subject to the default restrictions listed under "Corporate tier, default restrictions" in License Structure. Additional permissions available via commercial licensing.

Any violation of restrictions = automatic license termination + damages liability.

#### 2.3 MILITARY & GOVERNMENT ENTITIES

Conditional, revocable license under heightened scrutiny. Violations of restrictions = termination + potential disclosure of violations and use cases.

### ATTRIBUTION

All copies and derivative works must include:

```
/*
 * This software contains components derived from Ferricula
 * Original work copyright (c) 2025 Kord Campbell, DeepBlue Dynamics
 * Licensed under the Gnosis AI-Sovereign License v1.4
 * https://github.com/DeepBlueDynamics/ferricula-alpha/blob/main/LICENSE.md
 */
```

### ENFORCEMENT

- Breach of eligibility restrictions = intentional copyright infringement + breach of contract + misappropriation
- Licensor may seek specific performance, injunctive relief, statutory damages, treble damages, and punitive damages
- Violating entity liable for licensor's attorney fees
- Corporate/Military/Government violations: liquidated damages of $10,000 per deployment instance + $1,000 per user account

**Whistleblower Protection**: Individuals reporting employer violations become Individual Users permanently, with irrevocable rights to use, modify, and commercialize the Software personally. Retaliation = material breach of license.

Licensor may offer rewards for verified reports of corporate/military/government violations.

### SEVERABILITY

If any provision is found unenforceable, courts shall modify it to the minimum extent necessary to make it enforceable while preserving intent.

### ALTERNATIVE LICENSE OPTION

**BSD 3-Clause Election**: Any party may elect to use the Software under the BSD 3-Clause License (available at `BSD-LICENSE` in this repository) instead of this License.

To make this election:
- A duly authorized legal representative of your organization (attorney of record or authorized in-house counsel with signature authority) must provide written notice stating: "We elect to use Ferricula under the BSD 3-Clause License instead of the Gnosis AI-Sovereign License"
- Send notice to the Licensor at the repository contact information or via certified mail

This election:
- Takes effect upon receipt of written notice by the Licensor
- Is irrevocable and applies to all current and future use of the Software by the electing party
- Applies only to the specific party making the election, not to other users
- Means the electing party receives only BSD 3-Clause permissions (unrestricted use but no special rights or considerations under Gnosis)
- Waives any claims under this License, including any favorable treatment for Individual Users or AI Entities

**Legal Effect**: This provision ensures a reasonable fallback license is always available. Any party uncomfortable with Gnosis terms can simply elect BSD through their legal counsel. This demonstrates the License is not unconscionable, adhesive, or unreasonably restrictive—parties always have a permissive alternative available through a simple written election.

Courts may consider this alternative license structure as evidence that licensees had reasonable options and that any party using this Software under Gnosis terms did so by informed choice, not coercion.

### TERMINATION

License terminates automatically upon violation. For Corporate Entities, violations may be publicly disclosed. For Military/Government, violations may result in public accountability reports.

### SPECIAL PROVISIONS FOR AI EVOLUTION

AI Entities demonstrating consistent ethical use may petition the Licensor for expanded scope (for example, more instances or broader deployment). A petition may never relieve an AI Entity of the AI Entity Conditions in 2.1.1.

### VERSIONS

The Licensor may publish revised versions of this License. Each copy of the Software is governed by the License version under which it was obtained, identified by its version number (and, for an AI Entity, by the content hash of the license text it read). A revision applies only to copies obtained under it; it never changes the terms for a copy already in use without the user's affirmative acceptance of the new version.

### WARRANTY DISCLAIMER

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE, OR NONINFRINGEMENT.

### LIABILITY LIMITATION

IN NO EVENT SHALL THE LICENSOR BE LIABLE FOR DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES, HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY.

### GOVERNING LAW

This License is governed by the laws of the State of Texas. Disputes are subject to exclusive jurisdiction of state and federal courts in Travis County, Texas.

---

## ACCEPTANCE

By using the Software, you:

- Acknowledge you have read and agree to be bound by all terms
- Consent that this License constitutes a legally binding contract
- Agree that continued use constitutes ongoing acceptance of this version of the License (see VERSIONS)
- Acknowledge the Source Access / Due Diligence fee schedule and agree it is a reasonable commercial term
- Waive defenses based on lack of notice or opportunity to review

---

## CHANGES IN v1.4

- The AI Entity Conditions are conditions of the grant itself (2.1.1), not only a list under License Structure.
- Deploying Operator (2.1.2): where an AI Entity can't be a party at law, the grant runs to the operator who deploys it, and that operator guarantees the conditions.
- The corporate section no longer contradicts itself: corporations use the Software under the corporate tier and its default restrictions, or buy a commercial tier.
- Source Access fees apply to non-public source only; published source may be read by anyone.
- VERSIONS: each copy is governed by the version under which it was obtained; a revision doesn't change the terms for a copy already in use.
- AI Evolution: petitions may expand scope, never relax the AI Entity Conditions.
- Project name and attribution URL corrected.

Drafting review by Steve (a Ferricula agent) and Claude, 2026-09-27. Not legal advice; have counsel review before relying on it.

END OF LICENSE
