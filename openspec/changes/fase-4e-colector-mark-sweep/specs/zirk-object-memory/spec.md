## ADDED Requirements

### Requirement: La cabecera lleva la contabilidad del recolector

Todo objeto con identidad SHALL llevar, además del descriptor de despacho, la metadata que el recolector necesita para enumerar y liberar objetos inalcanzables — sin que ningún lector existente del descriptor de despacho necesite cambiar.

#### Scenario: El descriptor de despacho no cambia
- **WHEN** se inspecciona el campo de descriptor de despacho de un objeto tras crecer la cabecera
- **THEN** su valor y su formato son idénticos a los de antes de que la cabecera creciera

#### Scenario: Toda alocación es enumerable
- **WHEN** el recolector recorre el conjunto de objetos alocados
- **THEN** alcanza cada objeto vivo o muerto exactamente una vez, sin recorrer memoria no alocada por el runtime
