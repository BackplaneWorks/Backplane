<p align="left">
  <a href="mechanical.md"><img src="media/readme/language-en.svg" alt="English" width="92" height="28"></a>
</p>

# Proyectos mecánicos

Un proyecto de Backplane puede ser una carpeta común que contenga piezas mecánicas.
No necesita un proyecto de KiCad ni una placa. Los archivos y las herramientas de
modelado viven en la máquina que ejecuta el hub, incluso cuando usas un navegador o teléfono.

## Ver un modelo existente

1. Añade la carpeta del proyecto a Backplane y abre un hilo en ella.
2. Coloca los modelos que quieras en la lista de piezas dentro de una carpeta llamada `mech/`
   en la raíz del proyecto. Por ejemplo:

   ```text
   bracket-project/
     mech/
       bracket.step
       spacer.step
   ```

3. Abre la pestaña **Mech** del visor y selecciona una pieza para girarla en 3D.
4. Para imágenes fijas, abre **Renders** y luego **Crear renders**. Backplane
   genera vistas isométrica, frontal, superior y derecha bajo `mech/renders/`.
   La página de renders también tiene **Actualizar** para recargar la lista de piezas.

Los formatos de modelo compatibles son `.step`, `.stp` y `.glb`. La inspección de STEP
usa el asistente de conversión configurado; no se requiere ningún proyecto de KiCad.
Los documentos de FreeCAD (`.FCStd`) y las mallas STL no se muestran directamente;
expórtalos o conviértelos primero a un formato compatible.

Un modelo también puede quedar en otra parte del proyecto. Ábrelo desde **Archivos**,
o pídeselo al agente:

> Muestra `vendor/spacer.step` en la pestaña Mech sin modificarlo.

El agente puede usar `viewer_show` con la ruta absoluta del archivo y
`kind: "mech"`. El visor sigue los guardados de ese archivo. Cópialo o expórtalo
a `mech/` si también lo quieres en la lista de piezas del proyecto. La pestaña **3D**
es para la placa de KiCad; los modelos independientes pertenecen a **Mech**.

## Modelar una pieza con un agente

Instala FreeCAD en la máquina del hub para modelado y conversión de STEP.
Consulta [instalación](install.es.md) para la configuración por plataforma. Define
`BACKPLANE_FREECAD` apuntando a su ejecutable de consola al iniciar el hub si la detección
automática no lo encuentra. Los modelos GLB existentes se pueden ver sin conversión.

Empieza con una petición pequeña y concreta, por ejemplo:

> Crea una placa de montaje rectangular de 80 por 30 por 3 mm con dos taladros
> pasantes de 5 mm, centrados en su ancho y separados 60 mm a lo largo de su longitud.
> Centra el par en la placa. Mantén las dimensiones como parámetros con nombre
> en `mech/plate.py`, guarda `mech/plate.FCStd`, exporta `mech/plate.step`
> y muéstralo en la pestaña Mech.

Las instrucciones mecánicas de Backplane indican al agente que trabaje en milímetros,
con Z hacia arriba y el frente mirando hacia -Y, que compruebe la validez de la forma
y que mantenga juntos el script, el documento de FreeCAD y la exportación STEP. Pide
cambios en el script y regenera la exportación para ver cada revisión.

Conserva el código fuente y las exportaciones en Git si quieres seguir las revisiones
mecánicas. Las instantáneas por herramienta de KiCad del historial de diseño y la
comparación de placa o esquemático no ofrecen una comparación de geometría STEP.

Elige un [modelo mecánico principal](mechanical-model.md) cuando un proyecto
tenga varias piezas y la heurística de ensamblaje no sea la opción predeterminada preferida.
