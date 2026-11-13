import { expect, it } from 'vitest'

import { schematicWorkshopTarget } from './schematic-workshop'

const unsupported = (name: string, path = `schematics/${name}`) => ({
    kind: 'unsupported' as const,
    name,
    path,
})

it('offers the workshop for both schematic formats Studio cannot read', () => {
    expect(schematicWorkshopTarget(unsupported('castle.litematic'), 'instance')).toEqual({
        instanceId: 'instance',
        path: 'schematics/castle.litematic',
    })
    expect(schematicWorkshopTarget(unsupported('gear.schem'), 'instance')).toEqual({
        instanceId: 'instance',
        path: 'schematics/gear.schem',
    })
})

it('matches the extension regardless of case', () => {
    expect(schematicWorkshopTarget(unsupported('Castle.LITEMATIC'), 'instance')).not.toBeNull()
    expect(schematicWorkshopTarget(unsupported('Gear.Schem'), 'instance')).not.toBeNull()
})

it('leaves the legacy MCEdit format alone because the workshop rejects it', () => {
    expect(schematicWorkshopTarget(unsupported('house.schematic'), 'instance')).toBeNull()
})

it('leaves other binary files Studio cannot read alone', () => {
    expect(schematicWorkshopTarget(unsupported('server.jar'), 'instance')).toBeNull()
    expect(schematicWorkshopTarget(unsupported('region.mca'), 'instance')).toBeNull()
    expect(schematicWorkshopTarget(unsupported('icon.png'), 'instance')).toBeNull()
})

it('does not offer the workshop for a document Studio did open', () => {
    expect(
        schematicWorkshopTarget(
            { kind: 'text', name: 'castle.litematic', path: 'schematics/castle.litematic' },
            'instance',
        ),
    ).toBeNull()
})

it('needs an instance to resolve the file against', () => {
    expect(schematicWorkshopTarget(unsupported('castle.litematic'), undefined)).toBeNull()
    expect(schematicWorkshopTarget(undefined, 'instance')).toBeNull()
})
