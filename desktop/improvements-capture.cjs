// Capture only this window's own web contents. Chromium's capturePage omits
// native Browser child views, so composite those attached visible views in
// paint order using their supported capturePage and NativeImage APIs.
function snapshotChildren(window) {
  return (window.contentView?.children || []).filter(view => view.webContents && view.webContents !== window.webContents && view.getVisible())
    .map(view => ({view, contents:view.webContents, bounds:view.getBounds()}));
}
function sameChildren(window, children) {
  const now=snapshotChildren(window);
  return now.length===children.length&&now.every((child,i)=>child.view===children[i].view&&['x','y','width','height'].every(k=>child.bounds[k]===children[i].bounds[k]));
}
function over(base, size, layer, layerSize, bounds, content) {
  const left=Math.round(bounds.x*size.width/content.width),top=Math.round(bounds.y*size.height/content.height);
  const width=Math.round(bounds.width*size.width/content.width),height=Math.round(bounds.height*size.height/content.height);
  if(width<1||height<1)return;
  for(let y=Math.max(0,top);y<Math.min(size.height,top+height);y++)for(let x=Math.max(0,left);x<Math.min(size.width,left+width);x++){
    const source=(Math.min(layerSize.height-1,Math.floor((y-top)*layerSize.height/height))*layerSize.width+Math.min(layerSize.width-1,Math.floor((x-left)*layerSize.width/width)))*4;
    const dest=(y*size.width+x)*4,alpha=layer[source+3],rest=(255-alpha)/255;
    for(let c=0;c<3;c++)base[dest+c]=Math.min(255,Math.round(layer[source+c]+base[dest+c]*rest));
    base[dest+3]=Math.min(255,Math.round(alpha+base[dest+3]*rest));
  }
}
async function captureAppWindow(window, nativeImage) {
  if (!window || window.isDestroyed()) throw Error('The Backplane window is unavailable.');
  const children=snapshotChildren(window);
  if(children.length>8)throw Error('The app view changed. Capture again.');
  const image = await window.capturePage();
  if (image.isEmpty()) throw Error('The Backplane window could not be captured.');
  const size = image.getSize();
  if (size.width * size.height > 24000000) throw Error('This window is too large to capture.');
  let png;
  if(children.length){
    const content=window.getContentBounds(),pixels=Buffer.from(image.toBitmap());
    if(pixels.length!==size.width*size.height*4)throw Error('The app image could not be read.');
    for(const child of children){
      if(child.contents.isDestroyed()||child.contents.getOwnerBrowserWindow()!==window||!sameChildren(window,children))throw Error('The app view changed. Capture again.');
      const layer=await child.contents.capturePage(),layerSize=layer.getSize();
      if(layer.isEmpty()||layerSize.width*layerSize.height>24000000)throw Error('The Browser pane could not be captured.');
      const bitmap=layer.toBitmap();
      if(bitmap.length!==layerSize.width*layerSize.height*4)throw Error('The Browser image could not be read.');
      over(pixels,size,bitmap,layerSize,child.bounds,content);
    }
    if(!sameChildren(window,children))throw Error('The app view changed. Capture again.');
    nativeImage ||= require('electron').nativeImage;
    png=nativeImage.createFromBitmap(pixels,size).toPNG();
  }else png=image.toPNG();
  if (png.length > 32 * 1024 * 1024) throw Error('This capture is too large.');
  return 'data:image/png;base64,' + png.toString('base64');
}
module.exports = { captureAppWindow };
