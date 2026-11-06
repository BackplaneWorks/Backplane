// iOS adapter: Mac, simulator. Not wired: needs an XCUITest bundle or the app's debug action hook for taps and a control inventory.
export const name='ios';
export const capabilities={};
export async function run(j,ctx){return {journey:j.id,client:'ios',status:'blocked',blocked:['ios_driver'],metrics:{},notes:['xcrun simctl can launch, record and screenshot; taps and the dead-control sweep need XCUITest or the BACKPLANE_ACTS hook (mac-heavy lock)']}}
